mod last_expression;

use crate::formatter::python::ast::{Expr, NodeId, Operator, Other};
use crate::formatter::python::builders::parenthesize_if_expands;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::can_omit::can_omit_optional_parentheses;
use crate::formatter::python::expression::own_parentheses::{has_own_parentheses, has_parentheses};
use crate::formatter::python::expression::{format_expr, format_expr_preserving};
use crate::formatter::python::parentheses::{Parentheses, optional_parentheses};

pub use last_expression::{left_to_right, right_to_left};

const ASSIGN: &str = "=";

#[derive(Debug, Clone, Copy)]
pub enum AssignOperator {
    Assign,
    AugAssign(Operator),
}

impl AssignOperator {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assign => ASSIGN,
            Self::AugAssign(operator) => operator.as_augmented_str(),
        }
    }
}

pub fn has_target_own_parentheses(f: &Formatter<'_>, target: NodeId) -> bool {
    matches!(f.ast.expr(target), Some(Expr::Tuple { .. })) || has_own_parentheses(f, target).is_some()
}

fn call_has_arguments(f: &Formatter<'_>, arguments: NodeId) -> bool {
    match f.ast.other(arguments) {
        Some(Other::Arguments { args, keywords }) => !args.is_empty() || !keywords.is_empty(),
        Some(_) | None => false,
    }
}

fn is_attribute_with_parenthesized_value(f: &Formatter<'_>, target: NodeId) -> bool {
    match f.ast.expr(target) {
        Some(Expr::Attribute { value, .. }) => {
            has_parentheses(f, *value).is_some() || is_attribute_with_parenthesized_value(f, *value)
        }
        Some(Expr::Subscript { .. }) => true,
        Some(Expr::Call { arguments, .. }) => call_has_arguments(f, *arguments),
        Some(_) | None => false,
    }
}

// Un destino sin paréntesis propios (`a.b = ...`) puede ganar paréntesis si no entra en la línea.
fn should_parenthesize_target(f: &Formatter<'_>, target: NodeId) -> bool {
    !(has_target_own_parentheses(f, target) || is_attribute_with_parenthesized_value(f, target))
}

fn fmt_target_with_equal_operator(
    f: &mut Formatter<'_>,
    target: NodeId,
    preserve_parentheses: bool,
) -> Formatted {
    let comments = f.comments;
    if preserve_parentheses || comments.has_leading(target) || comments.has_trailing(target) {
        format_expr_preserving(f, target)?;
    } else if should_parenthesize_target(f, target) {
        parenthesize_if_expands(f, true, |f| format_expr(f, target, Parentheses::Never))?;
    } else {
        format_expr(f, target, Parentheses::Never)?;
    }
    f.space();
    f.token(ASSIGN);
    f.space();
    Ok(())
}

// Lo que va justo antes del operador cuando la sentencia se arma de derecha a izquierda.
pub fn fmt_before_operator(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let comments = f.comments;
    if comments.has_leading(expression) || comments.has_trailing(expression) {
        return format_expr_preserving(f, expression);
    }
    if !should_parenthesize_target(f, expression) {
        return format_expr(f, expression, Parentheses::Never);
    }
    if can_omit_optional_parentheses(f, expression) {
        optional_parentheses(f, |f| format_expr(f, expression, Parentheses::Never))
    } else {
        parenthesize_if_expands(f, true, |f| format_expr(f, expression, Parentheses::Never))
    }
}

// `a = b = valor`: el valor se acomoda primero y el último destino se parte sólo si hace falta.
pub fn fmt_assign(f: &mut Formatter<'_>, statement: NodeId, targets: &[NodeId], value: NodeId) -> Formatted {
    let Some((first, rest)) = targets.split_first() else {
        return Err(f.unsupported(statement, "asignación"));
    };
    if let Some((last, head)) = rest.split_last() {
        fmt_target_with_equal_operator(f, *first, true)?;
        for target in head {
            fmt_target_with_equal_operator(f, *target, false)?;
        }
        return right_to_left(f, (*last, AssignOperator::Assign), value, statement);
    }
    if has_target_own_parentheses(f, *first) && !f.is_expression_parenthesized(*first) {
        return right_to_left(f, (*first, AssignOperator::Assign), value, statement);
    }
    fmt_target_with_equal_operator(f, *first, true)?;
    left_to_right(f, value, statement)
}
