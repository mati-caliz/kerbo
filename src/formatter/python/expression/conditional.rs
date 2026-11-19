use super::{format_expr_preserving, format_node_with};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Ast, Expr, NodeId, Span, UnaryOperator};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{dangling_comments, leading_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{
    in_parentheses_only_group, in_parentheses_only_soft_line_break_or_space,
};
use crate::formatter::python::trivia::{Token, TokenKind, Tokenizer};

const COLON: &str = ":";
const SLICE: &str = "slice";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfExpLayout {
    Default,
    Nested,
}

fn fmt_orelse(f: &mut Formatter<'_>, orelse: NodeId) -> Formatted {
    let is_nested =
        matches!(f.ast.expr(orelse), Some(Expr::IfExp { .. })) && !f.is_expression_parenthesized(orelse);
    if is_nested {
        return format_node_with(f, orelse, |f| fmt_if_exp(f, orelse, IfExpLayout::Nested));
    }
    in_parentheses_only_group(f, |f| format_expr_preserving(f, orelse))
}

fn fmt_if_exp_parts(f: &mut Formatter<'_>, (test, body, orelse): (NodeId, NodeId, NodeId)) -> Formatted {
    let comments = f.comments;
    format_expr_preserving(f, body)?;
    in_parentheses_only_soft_line_break_or_space(f);
    leading_comments(f, comments.leading(test));
    f.token("if");
    f.space();
    format_expr_preserving(f, test)?;
    in_parentheses_only_soft_line_break_or_space(f);
    leading_comments(f, comments.leading(orelse));
    f.token("else");
    f.space();
    fmt_orelse(f, orelse)
}

// `a if b else c`: un `else` que es otra condicional sigue en el mismo grupo, sin anidarse.
pub fn fmt_if_exp(f: &mut Formatter<'_>, expression: NodeId, layout: IfExpLayout) -> Formatted {
    let Some(Expr::IfExp { test, body, orelse }) = f.ast.expr(expression) else {
        return Err(f.unsupported(expression, "expresión condicional"));
    };
    let parts = (*test, *body, *orelse);
    match layout {
        IfExpLayout::Default => in_parentheses_only_group(f, |f| fmt_if_exp_parts(f, parts)),
        IfExpLayout::Nested => fmt_if_exp_parts(f, parts),
    }
}

fn first_token_after_parentheses(source: &str, start: usize, end: usize) -> Option<Token> {
    Tokenizer::new(source, Span::new(start, end))
        .skip_trivia()
        .find(|token| token.kind != TokenKind::RParen)
}

// Los dos puntos de un slice: el primero siempre está, el segundo puede faltar.
pub fn find_colons(
    (ast, source): (&Ast, &str),
    slice: NodeId,
    (lower, upper): (Option<NodeId>, Option<NodeId>),
) -> Option<(Token, Option<Token>)> {
    let span = ast.span(slice);
    let after_lower = lower.map_or(span.start, |lower| ast.end(lower));
    let first_colon = first_token_after_parentheses(source, after_lower, span.end)
        .filter(|token| token.kind == TokenKind::Colon)?;
    let after_upper = upper.map_or(first_colon.span.end, |upper| ast.end(upper));
    match first_token_after_parentheses(source, after_upper, span.end) {
        Some(second_colon) if second_colon.kind == TokenKind::Colon => {
            Some((first_colon, Some(second_colon)))
        }
        Some(_) => None,
        None => Some((first_colon, None)),
    }
}

fn is_simple_expr(f: &Formatter<'_>, expression: NodeId) -> bool {
    match f.ast.expr(expression) {
        Some(Expr::UnaryOp { op, operand }) => *op != UnaryOperator::Not && is_simple_expr(f, *operand),
        Some(
            Expr::StringLiteral { .. }
            | Expr::BytesLiteral { .. }
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral
            | Expr::EllipsisLiteral
            | Expr::Name,
        ) => true,
        Some(_) | None => false,
    }
}

fn leading_comments_spacing(f: &mut Formatter<'_>, leading: &[SourceComment]) {
    let Some(first) = leading.first() else {
        return;
    };
    if first.is_own_line() {
        f.line(LineMode::Hard);
    } else {
        f.space();
        f.space();
    }
}

fn fmt_bound(f: &mut Formatter<'_>, bound: NodeId) -> Formatted {
    let leading = f.comments.leading(bound);
    leading_comments_spacing(f, leading);
    format_expr_preserving(f, bound)
}

fn split_at_colon(comments: &[SourceComment], colon: Option<Token>) -> (&[SourceComment], &[SourceComment]) {
    let before_the_colon = colon.map_or(comments.len(), |colon| {
        comments.partition_point(|comment| comment.span.start < colon.span.start)
    });
    comments.split_at(before_the_colon)
}

fn fmt_step(
    f: &mut Formatter<'_>,
    (upper, step): (Option<NodeId>, Option<NodeId>),
    all_simple: bool,
    dangling_step: &[SourceComment],
) -> Formatted {
    if !all_simple && (upper.is_some() || step.is_none()) {
        f.space();
    }
    f.token(COLON);
    if !all_simple && step.is_some() {
        f.space();
    }
    if let Some(step) = step {
        fmt_bound(f, step)?;
    } else if !dangling_step.is_empty() {
        f.line(LineMode::Hard);
        dangling_comments(f, dangling_step);
    }
    Ok(())
}

// `a[1:2]` va sin espacios; si alguna parte no es simple, `a[x + 1 : y]` los lleva alrededor de
// los dos puntos, como un operador.
pub fn fmt_slice(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let Some(Expr::Slice { lower, upper, step }) = f.ast.expr(expression) else {
        return Err(f.unsupported(expression, SLICE));
    };
    let (lower, upper, step) = (*lower, *upper, *step);
    let Some((first_colon, second_colon)) = find_colons((f.ast, f.source), expression, (lower, upper)) else {
        return Err(f.unsupported(expression, SLICE));
    };
    let dangling = f.comments.dangling(expression);
    let (dangling_lower, dangling_upper_step) = split_at_colon(dangling, Some(first_colon));
    let (dangling_upper, dangling_step) = split_at_colon(dangling_upper_step, second_colon);
    let all_simple = [lower, upper, step]
        .into_iter()
        .flatten()
        .all(|bound| is_simple_expr(f, bound));
    if let Some(lower) = lower {
        format_expr_preserving(f, lower)?;
        f.line_suffix_boundary();
    } else {
        dangling_comments(f, dangling_lower);
    }
    if !all_simple && lower.is_some() {
        f.space();
    }
    f.token(COLON);
    if !all_simple && upper.is_some() {
        f.space();
    }
    if let Some(upper) = upper {
        fmt_bound(f, upper)?;
        f.line_suffix_boundary();
    } else {
        if dangling_upper.first().is_some_and(SourceComment::is_own_line) {
            f.line(LineMode::Hard);
        }
        dangling_comments(f, dangling_upper);
    }
    if second_colon.is_some() {
        fmt_step(f, (upper, step), all_simple, dangling_step)?;
    }
    Ok(())
}
