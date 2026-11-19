use super::assign::{AssignOperator, has_target_own_parentheses, left_to_right, right_to_left};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, Operator, Span};
use crate::formatter::python::builders::{
    CommaSeparated, block_indent, parenthesize_if_expands, soft_block_indent,
};
use crate::formatter::python::comments::format::dangling_node_comments;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::own_parentheses::is_splittable_expression;
use crate::formatter::python::expression::{
    TupleParentheses, fmt_tuple, format_expr, format_expr_preserving, format_node_with,
    maybe_parenthesize_expression, needs_parentheses,
};
use crate::formatter::python::parentheses::{OptionalParentheses, Parentheses, Parenthesize};

const COMMA: &str = ",";
const LINE_CONTINUATION: &str = "\\";

fn fmt_name_separator(f: &mut Formatter<'_>, breaks_with_continuations: bool) {
    f.token(COMMA);
    f.space();
    if breaks_with_continuations {
        f.token_if_group_breaks(LINE_CONTINUATION);
        f.line(LineMode::Soft);
    }
}

pub fn fmt_return(f: &mut Formatter<'_>, statement: NodeId, value: Option<NodeId>) -> Formatted {
    f.token("return");
    let Some(value) = value else {
        return Ok(());
    };
    f.space();
    let is_tuple = matches!(f.ast.expr(value), Some(Expr::Tuple { .. }));
    if is_tuple && !f.comments.has_leading(value) {
        return format_node_with(f, value, |f| {
            fmt_tuple(f, value, TupleParentheses::OptionalParentheses)
        });
    }
    left_to_right(f, value, statement)
}

pub fn fmt_delete(f: &mut Formatter<'_>, statement: NodeId, targets: &[NodeId]) -> Formatted {
    f.token("del");
    f.space();
    match targets {
        [] => {
            f.token("(");
            block_indent(f, |f| {
                dangling_node_comments(f, statement);
                Ok(())
            })?;
            f.token(")");
            Ok(())
        }
        [single] => maybe_parenthesize_expression(f, *single, statement, Parenthesize::IfBreaks),
        _ => {
            let end = f.ast.end(statement);
            parenthesize_if_expands(f, true, |f| {
                let mut joiner = CommaSeparated::new(end);
                for target in targets {
                    joiner.entry(f, f.ast.end(*target), |f| format_expr_preserving(f, *target))?;
                }
                joiner.finish(f);
                Ok(())
            })
        }
    }
}

pub fn fmt_raise(
    f: &mut Formatter<'_>,
    statement: NodeId,
    exc: Option<NodeId>,
    cause: Option<NodeId>,
) -> Formatted {
    f.token("raise");
    if let Some(exc) = exc {
        f.space();
        maybe_parenthesize_expression(f, exc, statement, Parenthesize::Optional)?;
    }
    if let Some(cause) = cause {
        f.space();
        f.token("from");
        f.space();
        maybe_parenthesize_expression(f, cause, statement, Parenthesize::Optional)?;
    }
    Ok(())
}

pub fn fmt_assert(f: &mut Formatter<'_>, statement: NodeId, test: NodeId, msg: Option<NodeId>) -> Formatted {
    f.token("assert");
    f.space();
    maybe_parenthesize_expression(f, test, statement, Parenthesize::IfBreaks)?;
    if let Some(msg) = msg {
        f.token(COMMA);
        f.space();
        maybe_parenthesize_expression(f, msg, statement, Parenthesize::IfBreaksParenthesized)?;
    }
    Ok(())
}

// `global a, b`: no admite paréntesis, así que si no entra en la línea se parte con barras
// invertidas.
pub fn fmt_names(
    f: &mut Formatter<'_>,
    statement: NodeId,
    keyword: &'static str,
    names: &[Span],
) -> Formatted {
    f.token(keyword);
    f.space();
    let breaks_with_continuations = !f.comments.has_trailing(statement);
    let write_names = |formatter: &mut Formatter<'_>| {
        for (index, name) in names.iter().enumerate() {
            if index > 0 {
                fmt_name_separator(formatter, breaks_with_continuations);
            }
            formatter.source_text(*name);
        }
        Ok(())
    };
    if !breaks_with_continuations {
        return write_names(f);
    }
    f.start_group(None);
    f.token_if_group_breaks(LINE_CONTINUATION);
    f.line(LineMode::Soft);
    soft_block_indent(f, write_names)?;
    f.end_group();
    Ok(())
}

const fn is_arithmetic_like(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::BitOr
            | Operator::BitXor
            | Operator::LShift
            | Operator::RShift
            | Operator::Add
            | Operator::Sub
    )
}

pub fn fmt_expression(f: &mut Formatter<'_>, statement: NodeId, value: NodeId) -> Formatted {
    match f.ast.expr(value) {
        Some(Expr::BinOp { op, .. }) if is_arithmetic_like(*op) => {
            maybe_parenthesize_expression(f, value, statement, Parenthesize::Optional)
        }
        Some(_) | None => format_expr_preserving(f, value),
    }
}

pub fn fmt_aug_assign(
    f: &mut Formatter<'_>,
    statement: NodeId,
    (target, op): (NodeId, Operator),
    value: NodeId,
) -> Formatted {
    if has_target_own_parentheses(f, target) && !f.is_expression_parenthesized(target) {
        return right_to_left(f, (target, AssignOperator::AugAssign(op)), value, statement);
    }
    format_expr_preserving(f, target)?;
    f.space();
    f.token(op.as_augmented_str());
    f.space();
    left_to_right(f, value, statement)
}

pub fn fmt_ann_assign(
    f: &mut Formatter<'_>,
    statement: NodeId,
    (target, annotation): (NodeId, NodeId),
    value: Option<NodeId>,
) -> Formatted {
    let annotation_always_parenthesized =
        needs_parentheses(f, annotation, statement) == OptionalParentheses::Always;
    format_expr_preserving(f, target)?;
    f.token(":");
    f.space();
    let Some(value) = value else {
        if annotation_always_parenthesized {
            return format_expr(f, annotation, Parentheses::Always);
        }
        return left_to_right(f, annotation, statement);
    };
    if !annotation_always_parenthesized && is_splittable_expression(f, annotation) {
        return right_to_left(f, (annotation, AssignOperator::Assign), value, statement);
    }
    let comments = f.comments;
    let parentheses = if comments.has_leading(annotation)
        || comments.has_trailing(annotation)
        || annotation_always_parenthesized
    {
        Parentheses::Always
    } else {
        Parentheses::Never
    };
    format_expr(f, annotation, parentheses)?;
    f.space();
    f.token("=");
    f.space();
    left_to_right(f, value, statement)
}
