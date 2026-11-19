mod flat;
mod strings;

use super::format_expr;
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, Span, UnaryOperator};
use crate::formatter::python::comments::format::{leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{
    Parentheses, in_parentheses_only_group, in_parentheses_only_if_group_breaks,
    in_parentheses_only_soft_line_break, in_parentheses_only_soft_line_break_or_space,
};
use crate::formatter::python::trivia::{TokenKind, Tokenizer};
use flat::{Operand, OperatorPart, Part, after_operator, between_operators, first_operand, flatten};
use flat::{last_operand, lowest_precedence};

// `a + b`, `a < b` y `a and b` se formatean igual: se aplanan en una lista de operandos y
// operadores, y la línea se parte primero en los operadores de menor prioridad.
pub fn fmt_binary_like(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let parts = flatten(f, expression);
    let is_bool_op = matches!(f.ast.expr(expression), Some(Expr::BoolOp { .. }));
    let string_operands = if is_bool_op {
        Vec::new()
    } else {
        strings::implicit_concatenated_operands(f, &parts)
    };
    if string_operands.is_empty() {
        return in_parentheses_only_group(f, |f| fmt_slice(f, &parts));
    }
    strings::fmt_with_strings(f, &parts, &string_operands)
}

fn is_simple_power_operand(f: &Formatter<'_>, expression: NodeId) -> bool {
    match f.ast.expr(expression) {
        Some(Expr::UnaryOp { op, operand }) => {
            *op != UnaryOperator::Not && is_simple_power_operand(f, *operand)
        }
        Some(Expr::Attribute { value, .. }) => is_simple_power_operand(f, *value),
        Some(Expr::NumberLiteral(_) | Expr::NoneLiteral | Expr::BooleanLiteral(_) | Expr::Name) => true,
        Some(_) | None => false,
    }
}

// `x**2` va sin espacios si los dos operandos son simples.
fn is_simple_power_expression(f: &Formatter<'_>, left: NodeId, right: NodeId) -> bool {
    is_simple_power_operand(f, left)
        && is_simple_power_operand(f, right)
        && !f.is_expression_parenthesized(left)
        && !f.is_expression_parenthesized(right)
}

fn fmt_operator(f: &mut Formatter<'_>, operator: &OperatorPart<'_>) {
    f.token(operator.symbol.as_str());
    trailing_comments(f, operator.trailing_comments);
}

fn first_token_is(f: &Formatter<'_>, (start, end): (usize, usize), kind: TokenKind) -> bool {
    Tokenizer::new(f.source, Span::new(start, end))
        .skip_trivia()
        .next()
        .is_some_and(|token| token.kind == kind)
}

fn has_unparenthesized_leading_comments(f: &Formatter<'_>, operand: &Operand<'_>) -> bool {
    if let Some(leading) = operand.leading {
        return !leading.is_empty();
    }
    let leading = f.comments.leading(operand.expression);
    if !f.is_expression_parenthesized(operand.expression) {
        return !leading.is_empty();
    }
    let start = f.ast.start(operand.expression);
    leading.iter().any(|comment| {
        comment.is_unformatted() && first_token_is(f, (comment.span.end, start), TokenKind::LParen)
    })
}

// Un operando entre paréntesis los conserva. Los comentarios que quedaron afuera de sus paréntesis
// van afuera: antes del que abre o después del que cierra.
fn fmt_parenthesized_operand(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    let comments = f.comments;
    let (start, end) = (f.ast.start(expression), f.ast.end(expression));
    let leading = comments.leading(expression);
    let before_parentheses_end = leading
        .iter()
        .rposition(|comment| {
            comment.is_unformatted() && first_token_is(f, (comment.span.end, start), TokenKind::LParen)
        })
        .map_or(0, |position| position.saturating_add(1));
    let leading_before_parentheses = leading.get(..before_parentheses_end).unwrap_or_default();
    let trailing = comments.trailing(expression);
    let after_parentheses_start = trailing
        .iter()
        .position(|comment| {
            comment.is_unformatted() && first_token_is(f, (end, comment.span.start), TokenKind::RParen)
        })
        .unwrap_or(trailing.len());
    let trailing_after_parentheses = trailing.get(after_parentheses_start..).unwrap_or_default();
    for comment in trailing_after_parentheses {
        comment.mark_formatted();
    }
    leading_comments(f, leading_before_parentheses);
    format_expr(f, expression, Parentheses::Always)?;
    for comment in trailing_after_parentheses {
        comment.mark_unformatted();
    }
    trailing_comments(f, trailing_after_parentheses);
    Ok(())
}

fn fmt_operand(f: &mut Formatter<'_>, operand: &Operand<'_>) -> Formatted {
    if f.is_expression_parenthesized(operand.expression) {
        fmt_parenthesized_operand(f, operand.expression)
    } else {
        format_expr(f, operand.expression, Parentheses::Never)
    }
}

fn fmt_side(f: &mut Formatter<'_>, parts: &[Part<'_>]) -> Formatted {
    match parts {
        [Part::Operand(operand)] => fmt_operand(f, operand),
        _ => in_parentheses_only_group(f, |f| fmt_slice(f, parts)),
    }
}

fn fmt_left_and_operator(
    f: &mut Formatter<'_>,
    operator: &OperatorPart<'_>,
    left: &[Part<'_>],
    right: &[Part<'_>],
) -> Formatted {
    let (left_first, left_last) = (first_operand(left), last_operand(left));
    let right_first = first_operand(right);
    let is_pow = operator.symbol.is_pow()
        && left_last.zip(right_first).is_some_and(|(base, exponent)| {
            is_simple_power_expression(f, base.expression, exponent.expression)
        });
    if let Some(leading) = left_first.and_then(|operand| operand.leading) {
        leading_comments(f, leading);
    }
    fmt_side(f, left)?;
    if let Some(trailing) = left_last.and_then(|operand| operand.trailing) {
        trailing_comments(f, trailing);
    }
    if is_pow {
        in_parentheses_only_soft_line_break(f);
    } else {
        in_parentheses_only_soft_line_break_or_space(f);
    }
    fmt_operator(f, operator);
    let right_has_leading_comments =
        right_first.is_some_and(|operand| has_unparenthesized_leading_comments(f, operand));
    if operator.has_trailing_comments() || right_has_leading_comments {
        f.line(LineMode::Hard);
    } else if is_pow {
        in_parentheses_only_if_group_breaks(f, |f| {
            f.space();
            Ok(())
        })?;
    } else {
        f.space();
    }
    Ok(())
}

fn fmt_slice(f: &mut Formatter<'_>, parts: &[Part<'_>]) -> Formatted {
    if let [Part::Operand(operand)] = parts {
        return fmt_operand(f, operand);
    }
    let lowest = lowest_precedence(parts);
    let mut last_operator: Option<usize> = None;
    for (index, part) in parts.iter().enumerate() {
        let Part::Operator(operator) = part else {
            continue;
        };
        if operator.symbol.precedence() != lowest {
            continue;
        }
        let left = between_operators(parts, last_operator, index);
        fmt_left_and_operator(f, operator, left, after_operator(parts, index))?;
        last_operator = Some(index);
    }
    let right = last_operator.map_or(parts, |last| after_operator(parts, last));
    if let Some(leading) = first_operand(right).and_then(|operand| operand.leading) {
        leading_comments(f, leading);
    }
    fmt_side(f, right)
}
