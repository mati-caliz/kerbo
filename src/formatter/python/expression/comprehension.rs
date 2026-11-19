use super::collections::{TupleParentheses, fmt_tuple_node};
use super::{format_expr_preserving, format_node_with};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, Other, Span};
use crate::formatter::python::builders::group;
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{dangling_comments, leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::parenthesized;
use crate::formatter::python::trivia::{TokenKind, Tokenizer};

const PARENTHESES: (&str, &str) = ("(", ")");
const BRACKETS: (&str, &str) = ("[", "]");
const BRACES: (&str, &str) = ("{", "}");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorParentheses {
    Default,
    Preserve,
}

// Un espacio antes de la expresión, o un salto si tiene comentarios adelante y no va a quedar
// entre paréntesis.
fn spacer(f: &mut Formatter<'_>, expression: NodeId, preserve_parentheses: bool) {
    let will_be_parenthesized = preserve_parentheses && f.is_expression_parenthesized(expression);
    if f.comments.has_leading(expression) && !will_be_parenthesized {
        f.line(LineMode::SoftOrSpace);
    } else {
        f.space();
    }
}

fn split_before(
    comments: &[SourceComment],
    belongs_before: impl Fn(&SourceComment) -> bool,
) -> (&[SourceComment], &[SourceComment]) {
    comments.split_at(comments.partition_point(belongs_before))
}

fn in_keyword_start(f: &Formatter<'_>, target: NodeId, iter: NodeId) -> usize {
    Tokenizer::new(f.source, Span::new(f.ast.end(target), f.ast.start(iter)))
        .skip_trivia()
        .find(|token| token.kind == TokenKind::In)
        .map_or_else(|| f.ast.start(iter), |token| token.span.start)
}

fn fmt_ifs(f: &mut Formatter<'_>, ifs: &[NodeId], mut dangling: &[SourceComment]) -> Formatted {
    for if_case in ifs {
        let if_start = f.ast.start(*if_case);
        let (if_comments, rest) = split_before(dangling, |comment| comment.span.start < if_start);
        let (own_line, end_of_line) = split_before(if_comments, SourceComment::is_own_line);
        dangling = rest;
        f.line(LineMode::SoftOrSpace);
        leading_comments(f, own_line);
        f.token("if");
        trailing_comments(f, end_of_line);
        spacer(f, *if_case, true);
        format_expr_preserving(f, *if_case)?;
    }
    Ok(())
}

fn fmt_clause(f: &mut Formatter<'_>, clause: NodeId) -> Formatted {
    let Some(Other::Comprehension {
        is_async,
        target,
        iter,
        ifs,
    }) = f.ast.other(clause)
    else {
        return Err(f.unsupported(clause, "comprensión"));
    };
    if *is_async {
        f.token("async");
        f.space();
    }
    let (target_start, iter_start) = (f.ast.start(*target), f.ast.start(*iter));
    let in_start = in_keyword_start(f, *target, *iter);
    let dangling = f.comments.dangling(clause);
    let (before_target, dangling) = split_before(dangling, |comment| comment.span.end < target_start);
    let (before_in, dangling) = split_before(dangling, |comment| comment.span.end < in_start);
    let (trailing_in, dangling_ifs) = split_before(dangling, |comment| comment.span.start < iter_start);
    f.token("for");
    trailing_comments(f, before_target);
    let target_is_tuple = matches!(f.ast.expr(*target), Some(Expr::Tuple { .. }));
    spacer(f, *target, !target_is_tuple);
    if target_is_tuple {
        fmt_tuple_node(f, *target, TupleParentheses::Never)?;
    } else {
        format_expr_preserving(f, *target)?;
    }
    if before_in.is_empty() {
        f.space();
    } else {
        f.line(LineMode::SoftOrSpace);
    }
    leading_comments(f, before_in);
    f.token("in");
    trailing_comments(f, trailing_in);
    spacer(f, *iter, true);
    format_expr_preserving(f, *iter)?;
    fmt_ifs(f, ifs, dangling_ifs)
}

fn fmt_clauses(f: &mut Formatter<'_>, generators: &[NodeId]) -> Formatted {
    for (index, clause) in generators.iter().enumerate() {
        if index > 0 {
            f.line(LineMode::SoftOrSpace);
        }
        format_node_with(f, *clause, |f| fmt_clause(f, *clause))?;
    }
    Ok(())
}

fn fmt_element_and_clauses(f: &mut Formatter<'_>, elt: NodeId, generators: &[NodeId]) -> Formatted {
    group(f, |f| format_expr_preserving(f, elt))?;
    f.line(LineMode::SoftOrSpace);
    fmt_clauses(f, generators)
}

pub fn fmt_list_comp(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (elt, generators): (NodeId, &[NodeId]),
) -> Formatted {
    let dangling = f.comments.dangling(expression);
    parenthesized(f, BRACKETS, dangling, |f| {
        group(f, |f| fmt_element_and_clauses(f, elt, generators))
    })
}

pub fn fmt_set_comp(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (elt, generators): (NodeId, &[NodeId]),
) -> Formatted {
    let dangling = f.comments.dangling(expression);
    parenthesized(f, BRACES, dangling, |f| {
        group(f, |f| fmt_element_and_clauses(f, elt, generators))
    })
}

pub fn fmt_dict_comp(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (key, value): (NodeId, NodeId),
    generators: &[NodeId],
) -> Formatted {
    let key_start = f.ast.start(key);
    let dangling = f.comments.dangling(expression);
    let (open_parenthesis_comments, key_value_comments) =
        split_before(dangling, |comment| comment.span.end < key_start);
    parenthesized(f, BRACES, open_parenthesis_comments, |f| {
        group(f, |f| {
            group(f, |f| format_expr_preserving(f, key))?;
            f.token(":");
            if key_value_comments.is_empty() {
                f.space();
            } else {
                dangling_comments(f, key_value_comments);
            }
            format_expr_preserving(f, value)?;
            f.line(LineMode::SoftOrSpace);
            fmt_clauses(f, generators)
        })
    })
}

pub fn fmt_generator(
    f: &mut Formatter<'_>,
    expression: NodeId,
    parentheses: GeneratorParentheses,
) -> Formatted {
    let Some(Expr::Generator {
        elt,
        generators,
        parenthesized: is_parenthesized,
    }) = f.ast.expr(expression)
    else {
        return Err(f.unsupported(expression, "generador"));
    };
    let dangling = f.comments.dangling(expression);
    if parentheses == GeneratorParentheses::Preserve && dangling.is_empty() && !is_parenthesized {
        return fmt_element_and_clauses(f, *elt, generators);
    }
    parenthesized(f, PARENTHESES, dangling, |f| {
        group(f, |f| fmt_element_and_clauses(f, *elt, generators))
    })
}
