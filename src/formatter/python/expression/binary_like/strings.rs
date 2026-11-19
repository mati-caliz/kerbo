use super::flat::{
    Operand, OperatorPart, Part, after_operator, between_operators, first_operand, last_operand,
};
use super::{fmt_operator, fmt_slice, has_unparenthesized_leading_comments};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId};
use crate::formatter::python::comments::format::{leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::parentheses::{
    end_in_parentheses_only_group, in_parentheses_only_soft_line_break_or_space,
    start_in_parentheses_only_group,
};
use crate::formatter::python::string::implicit::fmt_implicit_concatenated_string;

#[derive(Debug, Clone, Copy)]
pub struct StringOperand {
    index: usize,
    string: NodeId,
}

fn is_implicit_concatenated_string(f: &Formatter<'_>, expression: NodeId) -> bool {
    match f.ast.expr(expression) {
        Some(Expr::StringLiteral { parts } | Expr::BytesLiteral { parts }) => parts.len() > 1,
        Some(_) | None => false,
    }
}

// Los operandos que son cadenas pegadas (concatenación implícita) sin paréntesis propios.
pub fn implicit_concatenated_operands(f: &Formatter<'_>, parts: &[Part<'_>]) -> Vec<StringOperand> {
    parts
        .iter()
        .enumerate()
        .filter_map(|(index, part)| match part {
            Part::Operand(operand) => Some(StringOperand {
                index,
                string: operand.expression,
            }),
            Part::Operator(_) => None,
        })
        .filter(|operand| {
            is_implicit_concatenated_string(f, operand.string)
                && !f.is_expression_parenthesized(operand.string)
        })
        .collect()
}

fn operand_at<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
    index: usize,
) -> Option<&'parts Operand<'comments>> {
    match parts.get(index) {
        Some(Part::Operand(operand)) => Some(operand),
        Some(Part::Operator(_)) | None => None,
    }
}

fn operator_at<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
    index: usize,
) -> Option<&'parts OperatorPart<'comments>> {
    match parts.get(index) {
        Some(Part::Operator(operator)) => Some(operator),
        Some(Part::Operand(_)) | None => None,
    }
}

fn fmt_string(f: &mut Formatter<'_>, string: NodeId) -> Formatted {
    let comments = f.comments;
    leading_comments(f, comments.leading(string));
    fmt_implicit_concatenated_string(f, string)?;
    trailing_comments(f, comments.trailing(string));
    Ok(())
}

// Lo que hay entre la cadena anterior y esta, con su operador, cierra el grupo que venía abierto.
fn fmt_before_string(
    f: &mut Formatter<'_>,
    parts: &[Part<'_>],
    last_operator: Option<usize>,
    left_operator_index: usize,
) -> Formatted {
    if last_operator == Some(left_operator_index) {
        end_in_parentheses_only_group(f);
        return Ok(());
    }
    let left = between_operators(parts, last_operator, left_operator_index);
    if let Some(leading) = first_operand(left).and_then(|operand| operand.leading) {
        leading_comments(f, leading);
    }
    fmt_slice(f, left)?;
    if let Some(trailing) = last_operand(left).and_then(|operand| operand.trailing) {
        trailing_comments(f, trailing);
    }
    in_parentheses_only_soft_line_break_or_space(f);
    let left_operator = operator_at(parts, left_operator_index);
    if let Some(left_operator) = left_operator {
        fmt_operator(f, left_operator);
    }
    end_in_parentheses_only_group(f);
    let operand_has_comments = operand_at(parts, left_operator_index.saturating_add(1))
        .is_some_and(|operand| has_unparenthesized_leading_comments(f, operand));
    if operand_has_comments || left_operator.is_some_and(OperatorPart::has_trailing_comments) {
        f.line(LineMode::Hard);
    } else {
        f.space();
    }
    Ok(())
}

fn fmt_string_operand(
    f: &mut Formatter<'_>,
    parts: &[Part<'_>],
    last_operator: Option<usize>,
    string: StringOperand,
) -> Formatted {
    let Some(left_operator_index) = string.index.checked_sub(1) else {
        return fmt_string(f, string.string);
    };
    fmt_before_string(f, parts, last_operator, left_operator_index)?;
    let operand = operand_at(parts, string.index);
    if let Some(leading) = operand.and_then(|operand| operand.leading) {
        leading_comments(f, leading);
    }
    fmt_string(f, string.string)?;
    if let Some(trailing) = operand.and_then(|operand| operand.trailing) {
        trailing_comments(f, trailing);
    }
    f.line_suffix_boundary();
    Ok(())
}

// El operador que sigue a una cadena abre el grupo de lo que viene después.
fn fmt_operator_after_string(f: &mut Formatter<'_>, parts: &[Part<'_>], right_operator_index: usize) -> bool {
    let Some(right_operator) = operator_at(parts, right_operator_index) else {
        return false;
    };
    start_in_parentheses_only_group(f);
    let right_operand = operand_at(parts, right_operator_index.saturating_add(1));
    let has_leading_comments =
        right_operand.is_some_and(|operand| has_unparenthesized_leading_comments(f, operand));
    if has_leading_comments {
        f.space();
    } else {
        in_parentheses_only_soft_line_break_or_space(f);
    }
    fmt_operator(f, right_operator);
    let is_unparenthesized =
        right_operand.is_some_and(|operand| !f.is_expression_parenthesized(operand.expression));
    if (has_leading_comments && is_unparenthesized) || right_operator.has_trailing_comments() {
        f.line(LineMode::Hard);
    } else {
        f.space();
    }
    true
}

// Una cadena pegada que es operando de `+` o `%` se parte de a una parte por línea sin sangrar el
// resto de la expresión: cada tramo entre dos de esas cadenas es su propio grupo.
pub fn fmt_with_strings(f: &mut Formatter<'_>, parts: &[Part<'_>], strings: &[StringOperand]) -> Formatted {
    start_in_parentheses_only_group(f);
    if strings.first().is_some_and(|first| first.index != 0) {
        start_in_parentheses_only_group(f);
    }
    let mut last_operator: Option<usize> = None;
    let mut ends_with_a_string = false;
    for string in strings {
        fmt_string_operand(f, parts, last_operator, *string)?;
        let right_operator_index = string.index.saturating_add(1);
        if !fmt_operator_after_string(f, parts, right_operator_index) {
            ends_with_a_string = true;
            break;
        }
        last_operator = Some(right_operator_index);
    }
    if let Some(last_operator) = last_operator
        && !ends_with_a_string
    {
        fmt_slice(f, after_operator(parts, last_operator))?;
        end_in_parentheses_only_group(f);
    }
    end_in_parentheses_only_group(f);
    Ok(())
}
