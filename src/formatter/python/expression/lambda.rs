use super::own_parentheses::has_own_parentheses;
use super::{format_expr, format_expr_preserving, format_node_with, needs_parentheses};
use crate::formatter::document::{BestFittingMode, LineMode, TagKind, remove_soft_line_breaks, will_break};
use crate::formatter::python::ast::{Expr, NodeId};
use crate::formatter::python::builders::{block_indent, parenthesize_if_expands};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{dangling_comments, leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{OptionalParentheses, Parentheses};
use crate::formatter::python::statement::parameters::{ParametersParentheses, fmt_parameters};

const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LambdaLayout {
    Default,
    Assignment,
}

// Los parámetros de una lambda no llevan paréntesis, así que no se pueden partir: se escriben sin
// saltos blandos, salvo que tengan comentarios adentro.
fn fmt_lambda_parameters(f: &mut Formatter<'_>, parameters: NodeId) -> Formatted {
    let fmt = |formatter: &mut Formatter<'_>| {
        format_node_with(formatter, parameters, |nested| {
            fmt_parameters(nested, parameters, ParametersParentheses::Never)
        })
    };
    if f.comments.contains_comments(parameters) {
        return fmt(f);
    }
    let start = f.len();
    fmt(f)?;
    let written = f.take_since(start);
    for element in remove_soft_line_breaks(&written) {
        f.push(element);
    }
    Ok(())
}

fn fmt_header<'comments>(
    f: &mut Formatter<'comments>,
    parameters: Option<NodeId>,
    dangling: &'comments [SourceComment],
) -> Formatted<&'comments [SourceComment]> {
    let Some(parameters) = parameters else {
        return Ok(dangling);
    };
    let parameters_start = f.ast.start(parameters);
    let (before_parameters, after_parameters) =
        dangling.split_at(dangling.partition_point(|comment| comment.span.end < parameters_start));
    if !before_parameters.is_empty() {
        dangling_comments(f, before_parameters);
    } else if f.comments.has_leading(parameters) {
        f.line(LineMode::Hard);
    } else {
        f.space();
    }
    fmt_lambda_parameters(f, parameters)?;
    Ok(after_parameters)
}

fn fmt_body_after_comments(
    f: &mut Formatter<'_>,
    body: NodeId,
    header_comments: &[SourceComment],
) -> Formatted {
    let first_own_line = header_comments
        .iter()
        .position(SourceComment::is_own_line)
        .unwrap_or(header_comments.len());
    let (trailing_header, leading_body) = header_comments.split_at(first_own_line);
    if f.is_expression_parenthesized(body) && f.comments.has_leading(body) {
        trailing_comments(f, header_comments);
        if leading_body.is_empty() {
            f.space();
        } else {
            f.line(LineMode::Hard);
        }
        return format_expr(f, body, Parentheses::Always);
    }
    f.space();
    f.token(OPENING_PARENTHESIS);
    trailing_comments(f, trailing_header);
    block_indent(f, |f| {
        leading_comments(f, leading_body);
        format_expr(f, body, Parentheses::Never)
    })?;
    f.token(CLOSING_PARENTHESIS);
    Ok(())
}

// Una llamada o un subíndice como cuerpo: entero en la línea, partido en sus propios paréntesis,
// o entre paréntesis nuevos, lo primero que entre.
fn fmt_call_like_body(f: &mut Formatter<'_>, body: NodeId) -> Formatted {
    let start = f.len();
    format_expr(f, body, Parentheses::Never)?;
    let unparenthesized = f.take_since(start);
    if will_break(&unparenthesized) {
        f.expand_parent();
    }
    let expanded_start = f.len();
    f.start_group_expanding(None, true);
    f.interned(&unparenthesized);
    f.end_group();
    let expanded = f.take_since(expanded_start);
    let parenthesized_start = f.len();
    f.token(OPENING_PARENTHESIS);
    block_indent(f, |f| {
        f.interned(&unparenthesized);
        Ok(())
    })?;
    f.token(CLOSING_PARENTHESIS);
    let parenthesized = f.take_since(parenthesized_start);
    f.best_fitting(
        vec![unparenthesized, expanded, parenthesized],
        BestFittingMode::FirstLine,
    );
    Ok(())
}

fn fmt_body(
    f: &mut Formatter<'_>,
    lambda: NodeId,
    body: NodeId,
    header_comments: &[SourceComment],
) -> Formatted {
    if !header_comments.is_empty() {
        return fmt_body_after_comments(f, body, header_comments);
    }
    let comments = f.comments;
    let needs = needs_parentheses(f, body, lambda);
    if comments.has_leading(body)
        || comments.has_trailing_own_line(body)
        || needs == OptionalParentheses::Always
    {
        return format_expr(f, body, Parentheses::Always);
    }
    if needs == OptionalParentheses::Multiline {
        return parenthesize_if_expands(f, true, |f| format_expr(f, body, Parentheses::Never));
    }
    if matches!(f.ast.expr(body), Some(Expr::Call { .. } | Expr::Subscript { .. })) {
        return fmt_call_like_body(f, body);
    }
    if has_own_parentheses(f, body).is_some() {
        return format_expr_preserving(f, body);
    }
    parenthesize_if_expands(f, true, |f| format_expr(f, body, Parentheses::Never))
}

pub fn fmt_lambda(f: &mut Formatter<'_>, expression: NodeId, layout: LambdaLayout) -> Formatted {
    let Some(Expr::Lambda { parameters, body }) = f.ast.expr(expression) else {
        return Err(f.unsupported(expression, "lambda"));
    };
    f.token("lambda");
    let dangling = f.comments.dangling(expression);
    let header_comments = fmt_header(f, *parameters, dangling)?;
    f.token(":");
    if header_comments.is_empty() {
        f.space();
    }
    if layout == LambdaLayout::Assignment {
        f.start_fits_expanded(None);
        fmt_body(f, expression, *body, header_comments)?;
        f.end(TagKind::FitsExpanded);
        return Ok(());
    }
    fmt_body(f, expression, *body, header_comments)
}
