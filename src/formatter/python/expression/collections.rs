use super::{format_expr_preserving, format_node_with};
use crate::formatter::python::ast::{Expr, NodeId, Span};
use crate::formatter::python::builders::{
    CommaSeparated, group, has_magic_trailing_comma, parenthesize_if_expands,
};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{dangling_comments, leading_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{empty_parenthesized, optional_parentheses, parenthesized};

const PARENTHESES: (&str, &str) = ("(", ")");
const BRACKETS: (&str, &str) = ("[", "]");
const BRACES: (&str, &str) = ("{", "}");
const COMMA: &str = ",";
const PAIR_OF_ELEMENTS: usize = 2;

// Qué hace una tupla con sus paréntesis según dónde está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TupleParentheses {
    Default,
    Preserve,
    Never,
    NeverPreserve,
    OptionalParentheses,
}

fn fmt_sequence(f: &mut Formatter<'_>, sequence_end: usize, elements: &[NodeId]) -> Formatted {
    let mut joiner = CommaSeparated::new(sequence_end);
    for element in elements {
        joiner.entry(f, f.ast.end(*element), |f| format_expr_preserving(f, *element))?;
    }
    joiner.finish(f);
    Ok(())
}

pub fn fmt_list(f: &mut Formatter<'_>, expression: NodeId, elts: &[NodeId]) -> Formatted {
    let dangling = f.comments.dangling(expression);
    if elts.is_empty() {
        return empty_parenthesized(f, BRACKETS, dangling);
    }
    let end = f.ast.end(expression);
    parenthesized(f, BRACKETS, dangling, |f| fmt_sequence(f, end, elts))
}

pub fn fmt_set(f: &mut Formatter<'_>, expression: NodeId, elts: &[NodeId]) -> Formatted {
    let dangling = f.comments.dangling(expression);
    let end = f.ast.end(expression);
    parenthesized(f, BRACES, dangling, |f| fmt_sequence(f, end, elts))
}

fn fmt_single_element(
    f: &mut Formatter<'_>,
    expression: NodeId,
    single: NodeId,
    preserve_missing_parentheses: bool,
) -> Formatted {
    if preserve_missing_parentheses {
        format_expr_preserving(f, single)?;
        let after_element = Span::new(f.ast.end(single), f.ast.end(expression));
        if has_magic_trailing_comma(f, after_element) {
            f.token(COMMA);
        }
        return Ok(());
    }
    let dangling = f.comments.dangling(expression);
    parenthesized(f, PARENTHESES, dangling, |f| {
        format_expr_preserving(f, single)?;
        f.token(COMMA);
        Ok(())
    })
}

fn fmt_unparenthesized(
    f: &mut Formatter<'_>,
    end: usize,
    elts: &[NodeId],
    parentheses: TupleParentheses,
) -> Formatted {
    match parentheses {
        TupleParentheses::Never => {
            for (index, element) in elts.iter().enumerate() {
                if index > 0 {
                    f.start_group(None);
                    f.token(COMMA);
                    f.space();
                    f.end_group();
                }
                format_expr_preserving(f, *element)?;
            }
            Ok(())
        }
        TupleParentheses::Preserve => group(f, |f| fmt_sequence(f, end, elts)),
        TupleParentheses::NeverPreserve => optional_parentheses(f, |f| fmt_sequence(f, end, elts)),
        TupleParentheses::OptionalParentheses if elts.len() == PAIR_OF_ELEMENTS => {
            optional_parentheses(f, |f| fmt_sequence(f, end, elts))
        }
        TupleParentheses::Default | TupleParentheses::OptionalParentheses => {
            parenthesize_if_expands(f, true, |f| fmt_sequence(f, end, elts))
        }
    }
}

pub fn fmt_tuple(f: &mut Formatter<'_>, expression: NodeId, parentheses: TupleParentheses) -> Formatted {
    let Some(Expr::Tuple {
        elts,
        parenthesized: is_parenthesized,
    }) = f.ast.expr(expression)
    else {
        return Err(f.unsupported(expression, "tupla"));
    };
    let dangling = f.comments.dangling(expression);
    let end = f.ast.end(expression);
    match elts.as_slice() {
        [] => empty_parenthesized(f, PARENTHESES, dangling),
        [single] => {
            let preserve_missing = parentheses == TupleParentheses::Preserve && !is_parenthesized;
            fmt_single_element(f, expression, *single, preserve_missing)
        }
        _ if *is_parenthesized
            && !(parentheses == TupleParentheses::NeverPreserve && dangling.is_empty()) =>
        {
            parenthesized(f, PARENTHESES, dangling, |f| fmt_sequence(f, end, elts))
        }
        _ => fmt_unparenthesized(f, end, elts, parentheses),
    }
}

fn pair_start(f: &Formatter<'_>, (key, value): (Option<NodeId>, NodeId)) -> usize {
    f.ast.start(key.unwrap_or(value))
}

fn fmt_key_value(
    f: &mut Formatter<'_>,
    (key, value): (Option<NodeId>, NodeId),
    comments: &[SourceComment],
) -> Formatted {
    let Some(key) = key else {
        let leading_value_comments = f.comments.leading(value);
        leading_comments(f, leading_value_comments);
        return group(f, |f| {
            f.token("**");
            format_expr_preserving(f, value)
        });
    };
    group(f, |f| {
        format_expr_preserving(f, key)?;
        f.token(":");
        if comments.is_empty() {
            f.space();
        } else {
            dangling_comments(f, comments);
        }
        format_expr_preserving(f, value)
    })
}

pub fn fmt_dict(f: &mut Formatter<'_>, expression: NodeId, items: &[(Option<NodeId>, NodeId)]) -> Formatted {
    let dangling = f.comments.dangling(expression);
    let Some(first) = items.first() else {
        return empty_parenthesized(f, BRACES, dangling);
    };
    let first_start = pair_start(f, *first);
    let (open_parenthesis_comments, mut key_value_comments) =
        dangling.split_at(dangling.partition_point(|comment| comment.span.end < first_start));
    let end = f.ast.end(expression);
    parenthesized(f, BRACES, open_parenthesis_comments, |f| {
        let mut joiner = CommaSeparated::new(end);
        for item in items {
            let pair_end = f.ast.end(item.1);
            let partition = key_value_comments.partition_point(|comment| comment.span.start < pair_end);
            let (pair_comments, rest) = key_value_comments.split_at(partition);
            key_value_comments = rest;
            joiner.entry(f, pair_end, |f| fmt_key_value(f, *item, pair_comments))?;
        }
        joiner.finish(f);
        Ok(())
    })
}

pub fn fmt_tuple_node(f: &mut Formatter<'_>, expression: NodeId, parentheses: TupleParentheses) -> Formatted {
    format_node_with(f, expression, |f| fmt_tuple(f, expression, parentheses))
}
