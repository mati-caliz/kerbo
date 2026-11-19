use super::number::{normalize_floating_number, normalize_integer};
use super::{format_expr, format_expr_preserving, maybe_parenthesize_expression};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, NumberKind, Operator, UnaryOperator};
use crate::formatter::python::comments::format::{dangling_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{
    Parentheses, Parenthesize, in_parentheses_only_soft_line_break_or_space,
};

const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const IMAGINARY_SUFFIX: &str = "j";

pub fn fmt_name(f: &mut Formatter<'_>, expression: NodeId) {
    f.node_source(expression);
}

pub fn fmt_boolean(f: &mut Formatter<'_>, value: bool) {
    f.token(if value { "True" } else { "False" });
}

pub fn fmt_await(f: &mut Formatter<'_>, expression: NodeId, value: NodeId) -> Formatted {
    f.token("await");
    f.space();
    maybe_parenthesize_expression(f, value, expression, Parenthesize::IfBreaks)
}

pub fn fmt_starred(f: &mut Formatter<'_>, expression: NodeId, value: NodeId) -> Formatted {
    f.token("*");
    let comments = f.comments;
    dangling_comments(f, comments.dangling(expression));
    format_expr_preserving(f, value)
}

pub fn fmt_number(f: &mut Formatter<'_>, expression: NodeId, kind: NumberKind) {
    let content = f.node_text(expression);
    match kind {
        NumberKind::Int => f.text(&normalize_integer(content)),
        NumberKind::Float => f.text(&normalize_floating_number(content)),
        NumberKind::Complex => {
            f.text(&normalize_floating_number(
                content.trim_end_matches(IMAGINARY_SUFFIXES),
            ));
            f.token(IMAGINARY_SUFFIX);
        }
    }
}

pub fn fmt_named(f: &mut Formatter<'_>, expression: NodeId, (target, value): (NodeId, NodeId)) -> Formatted {
    f.start_group(None);
    format_expr_preserving(f, target)?;
    in_parentheses_only_soft_line_break_or_space(f);
    f.end_group();
    f.token(":=");
    let dangling = f.comments.dangling(expression);
    if dangling.is_empty() {
        f.space();
    } else {
        dangling_comments(f, dangling);
        f.line(LineMode::Hard);
    }
    format_expr_preserving(f, value)
}

// Un comentario entre el operador y un operando sin paréntesis obliga a partir la línea.
pub fn unary_needs_line_break(f: &Formatter<'_>, operand: NodeId) -> bool {
    f.comments.has_leading(operand) && !f.is_expression_parenthesized(operand)
}

pub fn fmt_unary(f: &mut Formatter<'_>, expression: NodeId, op: UnaryOperator, operand: NodeId) -> Formatted {
    f.token(op.as_str());
    let comments = f.comments;
    trailing_comments(f, comments.dangling(expression));
    if unary_needs_line_break(f, operand) {
        f.line(LineMode::Hard);
    } else if op == UnaryOperator::Not {
        f.space();
    }
    let is_power = matches!(
        f.ast.expr(operand),
        Some(Expr::BinOp {
            op: Operator::Pow,
            ..
        })
    );
    if is_power {
        format_expr(f, operand, Parentheses::Always)
    } else {
        format_expr_preserving(f, operand)
    }
}

pub fn fmt_yield(
    f: &mut Formatter<'_>,
    expression: NodeId,
    keyword: &'static str,
    value: Option<NodeId>,
) -> Formatted {
    f.token(keyword);
    let Some(value) = value else {
        return Ok(());
    };
    f.space();
    maybe_parenthesize_expression(f, value, expression, Parenthesize::Optional)
}
