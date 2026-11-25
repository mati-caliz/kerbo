mod access;
mod binary_like;
pub mod call_chain;
pub mod can_omit;
mod collections;
mod comprehension;
pub mod conditional;
pub mod lambda;
pub mod left_most;
mod needs_parentheses;
mod number;
pub mod own_parentheses;
mod simple;

use super::ast::{Expr, NodeId, Span};
use super::builders::parenthesize_if_expands;
use super::comments::format::{leading_comments, trailing_comments};
use super::context::{Formatted, Formatter, NodeLevel};
use super::parentheses::{
    OptionalParentheses, Parentheses, Parenthesize, optional_parentheses, parenthesized,
};
use super::string;
use super::trivia::{BackwardsTokenizer, TokenKind, Tokenizer};
use crate::formatter::document::{Start, TagKind};
use call_chain::CallChainLayout;
use can_omit::can_omit_optional_parentheses;
use conditional::IfExpLayout;
use lambda::LambdaLayout;

pub use access::fmt_arguments;
pub use collections::{TupleParentheses, fmt_tuple};
pub use comprehension::{GeneratorParentheses, fmt_generator};
pub use needs_parentheses::needs_parentheses;

const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

// `FormatNodeRule::fmt` de ruff: los comentarios de antes, el nodo y los de después.
pub fn format_node_with(
    f: &mut Formatter<'_>,
    node: NodeId,
    fields: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let comments = f.comments;
    if !comments.has(node) {
        return fields(f);
    }
    leading_comments(f, comments.leading(node));
    fields(f)?;
    trailing_comments(f, comments.trailing(node));
    Ok(())
}

fn fmt_fields(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let Some(data) = f.ast.expr(expression) else {
        return Err(f.unsupported(expression, "expresión"));
    };
    match data {
        Expr::BoolOp { .. } | Expr::BinOp { .. } | Expr::Compare { .. } => {
            binary_like::fmt_binary_like(f, expression)
        }
        Expr::Named { target, value } => simple::fmt_named(f, expression, (*target, *value)),
        Expr::UnaryOp { op, operand } => simple::fmt_unary(f, expression, *op, *operand),
        Expr::Await { value } => simple::fmt_await(f, expression, *value),
        Expr::Yield { value } => simple::fmt_yield(f, expression, "yield", *value),
        Expr::YieldFrom { value } => simple::fmt_yield(f, expression, "yield from", Some(*value)),
        Expr::Starred { value } => simple::fmt_starred(f, expression, *value),
        Expr::Lambda { .. } => lambda::fmt_lambda(f, expression, LambdaLayout::Default),
        Expr::IfExp { .. } => conditional::fmt_if_exp(f, expression, IfExpLayout::Default),
        Expr::Slice { .. } => conditional::fmt_slice(f, expression),
        Expr::Dict { items } => collections::fmt_dict(f, expression, items),
        Expr::Set { elts } => collections::fmt_set(f, expression, elts),
        Expr::List { elts } => collections::fmt_list(f, expression, elts),
        Expr::Tuple { .. } => fmt_tuple(f, expression, TupleParentheses::Default),
        Expr::ListComp { elt, generators } => comprehension::fmt_list_comp(f, expression, (*elt, generators)),
        Expr::SetComp { elt, generators } => comprehension::fmt_set_comp(f, expression, (*elt, generators)),
        Expr::DictComp {
            key,
            value,
            generators,
        } => comprehension::fmt_dict_comp(f, expression, (*key, *value), generators),
        Expr::Generator { .. } => fmt_generator(f, expression, GeneratorParentheses::Default),
        Expr::Call { .. } | Expr::Attribute { .. } | Expr::Subscript { .. } => {
            access::fmt_fields(f, expression, CallChainLayout::Default)
        }
        Expr::StringLiteral { .. } | Expr::BytesLiteral { .. } => {
            string::fmt_string_expression(f, expression)
        }
        Expr::Name
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral => {
            fmt_literal(f, expression, data);
            Ok(())
        }
    }
}

fn fmt_literal(f: &mut Formatter<'_>, expression: NodeId, data: &Expr) {
    if let Expr::NumberLiteral(kind) = data {
        simple::fmt_number(f, expression, *kind);
    } else if let Expr::BooleanLiteral(value) = data {
        simple::fmt_boolean(f, *value);
    } else if matches!(data, Expr::NoneLiteral) {
        f.token("None");
    } else if matches!(data, Expr::EllipsisLiteral) {
        f.token("...");
    } else {
        simple::fmt_name(f, expression);
    }
}

pub fn format_expression_node(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    format_node_with(f, expression, |f| fmt_fields(f, expression))
}

// Los paréntesis de más afuera que rodean a la expresión, mirando sólo los tokens: los de una
// llamada con un solo argumento también cuentan, igual que en ruff.
fn outermost_parentheses(f: &Formatter<'_>, expression: NodeId) -> Option<Span> {
    let span = f.ast.span(expression);
    let closing = Tokenizer::starts_at(span.end, f.source)
        .skip_trivia()
        .take_while(|token| token.kind == TokenKind::RParen);
    let opening = BackwardsTokenizer::up_to(span.start, f.source, f.comment_ranges)
        .skip_trivia()
        .take_while(|token| token.kind == TokenKind::LParen);
    closing
        .zip(opening)
        .last()
        .map(|(right, left)| Span::new(left.span.start, right.span.end))
}

// `format_with_parentheses_comments` de ruff: los comentarios de afuera de los paréntesis quedan
// afuera y los de adentro, adentro; el del final de la línea del paréntesis que abre sigue ahí.
fn format_with_parentheses_comments(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let comments = f.comments;
    let leading = comments.leading(expression);
    let trailing = comments.trailing(expression);
    let (leading_split, trailing_split) =
        outermost_parentheses(f, expression).map_or((0, trailing.len()), |parentheses| {
            (
                leading.partition_point(|comment| comment.span.start < parentheses.start),
                trailing.partition_point(|comment| comment.span.start < parentheses.end),
            )
        });
    let (leading_outer, leading_inner) = leading.split_at(leading_split);
    let (trailing_inner, trailing_outer) = trailing.split_at(trailing_split);
    let (parentheses_comment, leading_inner) = match leading_inner.split_first() {
        Some((first, rest)) if !first.is_own_line() => (std::slice::from_ref(first), rest),
        Some(_) | None => (<&[_]>::default(), leading),
    };
    leading_comments(f, leading_outer);
    parenthesized(
        f,
        (OPENING_PARENTHESIS, CLOSING_PARENTHESIS),
        parentheses_comment,
        |f| {
            leading_comments(f, leading_inner);
            fmt_fields(f, expression)?;
            trailing_comments(f, trailing_inner);
            Ok(())
        },
    )?;
    trailing_comments(f, trailing_outer);
    Ok(())
}

// `FormatExpr` de ruff: la expresión con sus paréntesis, si los conserva o se los piden.
pub fn format_expr(f: &mut Formatter<'_>, expression: NodeId, parentheses: Parentheses) -> Formatted {
    let parenthesize = match parentheses {
        Parentheses::Preserve => f.is_expression_parenthesized(expression),
        Parentheses::Always => true,
        Parentheses::Never => false,
    };
    if parenthesize {
        let comments = f.comments;
        if comments.has_leading(expression) || comments.has_trailing(expression) {
            return format_with_parentheses_comments(f, expression);
        }
        return parenthesized(f, (OPENING_PARENTHESIS, CLOSING_PARENTHESIS), &[], |f| {
            format_expression_node(f, expression)
        });
    }
    let level = match f.node_level {
        NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement => NodeLevel::Expression(None),
        saved @ (NodeLevel::Expression(_) | NodeLevel::ParenthesizedExpression) => saved,
    };
    let saved_level = f.enter(level);
    format_expression_node(f, expression)?;
    f.node_level = saved_level;
    Ok(())
}

pub fn format_expr_preserving(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    format_expr(f, expression, Parentheses::Preserve)
}

fn fmt_optional_or_expanding(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    if can_omit_optional_parentheses(f, expression) {
        optional_parentheses(f, |f| format_expr(f, expression, Parentheses::Never))
    } else {
        parenthesize_if_expands(f, true, |f| format_expr(f, expression, Parentheses::Never))
    }
}

fn fmt_best_fit(f: &mut Formatter<'_>, expression: NodeId, parenthesize: Parenthesize) -> Formatted {
    match parenthesize {
        Parenthesize::IfBreaksParenthesized | Parenthesize::IfBreaksParenthesizedNested => {
            fmt_optional_or_expanding(f, expression)
        }
        Parenthesize::Optional | Parenthesize::IfRequired => format_expr(f, expression, Parentheses::Never),
        Parenthesize::IfBreaks => {
            if f.comments.has_trailing(expression) {
                return format_expr(f, expression, Parentheses::Always);
            }
            let group = f.group_id();
            let saved_level = f.enter(NodeLevel::Expression(Some(group)));
            f.start(Start::BestFitParenthesize { id: Some(group) });
            format_expr(f, expression, Parentheses::Never)?;
            f.end(TagKind::BestFitParenthesize);
            f.node_level = saved_level;
            Ok(())
        }
    }
}

// La expresión con los paréntesis que le hagan falta según lo que pide ella (`needs_parentheses`)
// y lo que deja el padre (`parenthesize`).
pub fn maybe_parenthesize_expression(
    f: &mut Formatter<'_>,
    expression: NodeId,
    parent: NodeId,
    parenthesize: Parenthesize,
) -> Formatted {
    let preserve_parentheses =
        parenthesize == Parenthesize::Optional && f.is_expression_parenthesized(expression);
    let comments = f.comments;
    if preserve_parentheses || comments.has_leading(expression) || comments.has_trailing_own_line(expression)
    {
        return format_expr(f, expression, Parentheses::Always);
    }
    let needs = needs_parentheses(f, expression, parent);
    if needs != OptionalParentheses::Always && f.node_level.is_parenthesized() {
        if parenthesize == Parenthesize::IfBreaksParenthesizedNested {
            return parenthesize_if_expands(f, true, |f| format_expr(f, expression, Parentheses::Never));
        }
        return format_expr(f, expression, Parentheses::Never);
    }
    match needs {
        OptionalParentheses::Multiline => {
            if parenthesize == Parenthesize::IfRequired {
                format_expr(f, expression, Parentheses::Never)
            } else {
                fmt_optional_or_expanding(f, expression)
            }
        }
        OptionalParentheses::BestFit => fmt_best_fit(f, expression, parenthesize),
        OptionalParentheses::Never => format_expr(f, expression, Parentheses::Never),
        OptionalParentheses::Always => format_expr(f, expression, Parentheses::Always),
    }
}
