mod arguments;

use super::call_chain::CallChainLayout;
use super::collections::{TupleParentheses, fmt_tuple};
use super::{format_expr, format_expr_preserving, format_node_with};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, NumberKind, Span};
use crate::formatter::python::comments::format::dangling_comments;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{Parentheses, parenthesized};
use crate::formatter::python::trivia::{TokenKind, Tokenizer};

pub use arguments::fmt_arguments;

const DOT: &str = ".";
const NON_DECIMAL_PREFIXES: [&str; 6] = ["0x", "0X", "0o", "0O", "0b", "0B"];

// Un eslabón de una cadena `a.b().c[0]`, con el estilo que decidió el primero.
pub fn fmt_chain_node(f: &mut Formatter<'_>, node: NodeId, layout: CallChainLayout) -> Formatted {
    format_node_with(f, node, |f| fmt_fields(f, node, layout))
}

pub fn fmt_fields(f: &mut Formatter<'_>, expression: NodeId, layout: CallChainLayout) -> Formatted {
    match f.ast.expr(expression) {
        Some(Expr::Call { func, arguments }) => fmt_call(f, expression, (*func, *arguments), layout),
        Some(Expr::Subscript { value, slice }) => fmt_subscript(f, expression, (*value, *slice), layout),
        Some(Expr::Attribute { value, attr }) => fmt_attribute(f, expression, (*value, *attr), layout),
        Some(_) | None => Err(f.unsupported(expression, "expresión")),
    }
}

// Lo que está a la izquierda de un `.`, un `(` o un `[`: si es otro eslabón, sigue la cadena.
fn fmt_chain_value(
    f: &mut Formatter<'_>,
    value: NodeId,
    (after_attribute, after_call): (CallChainLayout, CallChainLayout),
) -> Formatted {
    if f.is_expression_parenthesized(value) {
        return format_expr(f, value, Parentheses::Always);
    }
    match f.ast.expr(value) {
        Some(Expr::Attribute { .. }) => fmt_chain_node(f, value, after_attribute),
        Some(Expr::Call { .. } | Expr::Subscript { .. }) => fmt_chain_node(f, value, after_call),
        Some(_) | None => format_expr(f, value, Parentheses::Never),
    }
}

fn in_root_group(
    f: &mut Formatter<'_>,
    is_root: bool,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    if !is_root {
        return content(f);
    }
    f.start_group(None);
    content(f)?;
    f.end_group();
    Ok(())
}

fn fmt_call(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (func, arguments): (NodeId, NodeId),
    own_layout: CallChainLayout,
) -> Formatted {
    let layout = own_layout.apply_in_node(f, expression);
    let is_root = layout.is_fluent() && own_layout == CallChainLayout::Default;
    in_root_group(f, is_root, |f| {
        fmt_chain_value(f, func, (layout.decrement_call_like_count(), layout))?;
        let comments = f.comments;
        dangling_comments(f, comments.dangling(expression));
        format_node_with(f, arguments, |f| fmt_arguments(f, arguments))
    })
}

fn fmt_subscript(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (value, slice): (NodeId, NodeId),
    own_layout: CallChainLayout,
) -> Formatted {
    let layout = own_layout.apply_in_node(f, expression);
    let is_root = layout.is_fluent() && own_layout == CallChainLayout::Default;
    in_root_group(f, is_root, |f| {
        fmt_chain_value(f, value, (layout.decrement_call_like_count(), layout))?;
        let dangling = f.comments.dangling(expression);
        parenthesized(f, ("[", "]"), dangling, |f| {
            if matches!(f.ast.expr(slice), Some(Expr::Tuple { .. })) {
                format_node_with(f, slice, |f| fmt_tuple(f, slice, TupleParentheses::Preserve))
            } else {
                format_expr_preserving(f, slice)
            }
        })
    })
}

// `1.real` no es válido: el número necesita paréntesis antes del punto.
fn is_base_ten_number_literal(f: &Formatter<'_>, expression: NodeId) -> bool {
    match f.ast.expr(expression) {
        Some(Expr::NumberLiteral(NumberKind::Float)) => true,
        Some(Expr::NumberLiteral(NumberKind::Int)) => {
            let text = f.node_text(expression);
            !NON_DECIMAL_PREFIXES.iter().any(|prefix| text.starts_with(prefix))
        }
        Some(_) | None => false,
    }
}

// Un comentario al final de la línea después del paréntesis que cierra el valor: el punto tiene
// que bajar a la línea siguiente.
fn has_trailing_end_of_line_comment(f: &Formatter<'_>, value: NodeId) -> bool {
    let right_parenthesis = Tokenizer::starts_at(f.ast.end(value), f.source)
        .skip_trivia()
        .take_while(|token| token.kind == TokenKind::RParen)
        .last();
    right_parenthesis.is_some_and(|right_parenthesis| {
        f.comments
            .trailing(value)
            .iter()
            .any(|comment| !comment.is_own_line() && comment.span.start > right_parenthesis.span.end)
    })
}

fn dot_start(f: &Formatter<'_>, value: NodeId, attr: Span) -> usize {
    Tokenizer::new(f.source, Span::new(f.ast.end(value), attr.start))
        .skip_trivia()
        .find(|token| token.kind == TokenKind::Dot)
        .map_or(attr.start, |dot| dot.span.start)
}

fn fmt_dot_and_name(f: &mut Formatter<'_>, expression: NodeId, (value, attr): (NodeId, Span)) {
    let dangling = f.comments.dangling(expression);
    let before_name = Span::new(attr.start.saturating_sub(DOT.len()), attr.end);
    if dangling.is_empty() && f.text_of(before_name).starts_with(DOT) {
        f.source_text(before_name);
        return;
    }
    let dot = dot_start(f, value, attr);
    let (before_dot, after_dot) =
        dangling.split_at(dangling.partition_point(|comment| comment.span.start < dot));
    dangling_comments(f, before_dot);
    f.token(DOT);
    dangling_comments(f, after_dot);
    f.source_text(attr);
}

fn fmt_attribute_value(f: &mut Formatter<'_>, value: NodeId, layout: CallChainLayout) -> Formatted {
    let parenthesize_value = is_base_ten_number_literal(f, value) || f.is_expression_parenthesized(value);
    if parenthesize_value {
        format_expr(f, value, Parentheses::Always)?;
    } else if layout.is_fluent() {
        let next = layout.transition_after_attribute();
        fmt_chain_value(f, value, (next, next))?;
    } else {
        format_expr(f, value, Parentheses::Never)?;
    }
    if has_trailing_end_of_line_comment(f, value) {
        f.line(LineMode::Hard);
    } else if layout.is_fluent() {
        let is_call_like = matches!(
            f.ast.expr(value),
            Some(Expr::Call { .. } | Expr::Subscript { .. })
        );
        if parenthesize_value || is_call_like {
            f.line(LineMode::Soft);
        }
    }
    Ok(())
}

fn fmt_attribute(
    f: &mut Formatter<'_>,
    expression: NodeId,
    (value, attr): (NodeId, Span),
    own_layout: CallChainLayout,
) -> Formatted {
    let layout = own_layout.apply_in_node(f, expression);
    let comments = f.comments;
    let is_plain_name = matches!(f.ast.expr(value), Some(Expr::Name))
        && !layout.is_fluent()
        && f.text_of(Span::new(f.ast.end(value), attr.start)) == DOT
        && !comments.has(value)
        && !comments.has_dangling(expression);
    if is_plain_name {
        f.node_source(expression);
        return Ok(());
    }
    let is_root = own_layout == CallChainLayout::Default && layout.is_fluent();
    in_root_group(f, is_root, |f| {
        fmt_attribute_value(f, value, layout)?;
        fmt_dot_and_name(f, expression, (value, attr));
        Ok(())
    })
}
