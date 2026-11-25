use super::assign::{AssignOperator, right_to_left};
use crate::formatter::python::ast::{Expr, NodeId, Other, Stmt};
use crate::formatter::python::builders::CommaSeparated;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::{format_expr, format_expr_preserving, format_node_with};
use crate::formatter::python::parentheses::{Parentheses, parenthesized};
use crate::formatter::python::statement::assign::left_to_right;

const OPENING_BRACKET: &str = "[";
const CLOSING_BRACKET: &str = "]";
const TYPE: &str = "type";
const COLON: &str = ":";
const ASSIGN: &str = "=";
const STAR: &str = "*";
const DOUBLE_STAR: &str = "**";
const TYPE_ALIAS: &str = "alias de tipo";

pub fn fmt_optional_type_params(f: &mut Formatter<'_>, type_params: Option<NodeId>) -> Formatted {
    type_params.map_or(Ok(()), |node| fmt_type_params(f, node))
}

fn fmt_default(f: &mut Formatter<'_>, default: Option<NodeId>) -> Formatted {
    let Some(default) = default else {
        return Ok(());
    };
    f.space();
    f.token(ASSIGN);
    f.space();
    format_expr_preserving(f, default)
}

fn fmt_param(f: &mut Formatter<'_>, param: NodeId) -> Formatted {
    match f.ast.other(param) {
        Some(Other::TypeVar { name, bound, default }) => {
            f.source_text(*name);
            if let Some(bound) = bound {
                f.token(COLON);
                f.space();
                format_expr_preserving(f, *bound)?;
            }
            fmt_default(f, *default)
        }
        Some(Other::TypeVarTuple { name, default }) => {
            f.token(STAR);
            f.source_text(*name);
            fmt_default(f, *default)
        }
        Some(Other::ParamSpec { name, default }) => {
            f.token(DOUBLE_STAR);
            f.source_text(*name);
            fmt_default(f, *default)
        }
        Some(_) | None => Err(f.unsupported(param, "parámetro de tipo")),
    }
}

fn fmt_entries(f: &mut Formatter<'_>, node: NodeId, params: &[NodeId]) -> Formatted {
    let mut joiner = CommaSeparated::new(f.ast.end(node));
    for param in params {
        let end = f.ast.end(*param);
        joiner.entry(f, end, |entry| {
            format_node_with(entry, *param, |inner| fmt_param(inner, *param))
        })?;
    }
    joiner.finish(f);
    Ok(())
}

pub fn fmt_type_params(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::TypeParams { params }) = f.ast.other(node) else {
        return Err(f.unsupported(node, "parámetros de tipo"));
    };
    let dangling = f.comments.dangling(node);
    format_node_with(f, node, |f| {
        parenthesized(f, (OPENING_BRACKET, CLOSING_BRACKET), dangling, |nested| {
            fmt_entries(nested, node, params)
        })
    })
}

fn is_invalid_type_expression(f: &Formatter<'_>, value: NodeId) -> bool {
    matches!(
        f.ast.expr(value),
        Some(Expr::Named { .. } | Expr::Await { .. } | Expr::Yield { .. } | Expr::YieldFrom { .. })
    )
}

pub fn fmt_type_alias(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::TypeAlias {
        name,
        type_params,
        value,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, TYPE_ALIAS));
    };
    f.token(TYPE);
    f.space();
    format_expr_preserving(f, *name)?;
    if is_invalid_type_expression(f, *value) {
        fmt_optional_type_params(f, *type_params)?;
        f.space();
        f.token(ASSIGN);
        f.space();
        return format_expr(f, *value, Parentheses::Preserve);
    }
    if let Some(type_params) = type_params {
        return right_to_left(f, (*type_params, AssignOperator::Assign), *value, statement);
    }
    f.space();
    f.token(ASSIGN);
    f.space();
    left_to_right(f, *value, statement)
}
