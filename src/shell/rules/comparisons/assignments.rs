use tree_sitter::Node;

use super::NumericValues;
use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::reference;
use crate::shell::syntax::{
    ARRAY, ASSIGNMENT, ASSIGNMENTS, COMMAND, DECLARATION, EXPANSIONS, NAME_FIELD, VALUE_FIELD, children,
    named_children, text,
};
use crate::shell::variables::{EventKind, flow};
use crate::shell::word_pieces::{PieceKind, pieces, unquoted_literal};
use crate::shell::words::braced_content;

const APPEND: &str = "+=";
const ELEMENT_COMMA: char = ',';
const INDEX_OPEN: char = '[';
const INDEX_VALUE: &str = "]=";
const ARITHMETIC_SIGNS: &[char] = &['+', '*', '-'];
const NAME_EXTRA: char = '_';

pub fn check(
    node: Node<'_>,
    context: &Context<'_, '_>,
    numeric_values: &NumericValues,
    findings: &mut Findings,
) {
    if node.kind() == ARRAY {
        check_commas(node, context.source, findings);
    } else if node.kind() == ASSIGNMENT {
        check_append_number(node, context, numeric_values, findings);
        check_unexpanded_arithmetic(node, context, findings);
    }
}

// SC2054: `checkCommarrays`.
fn check_commas(array: Node<'_>, source: &[u8], findings: &mut Findings) {
    let has_comma = named_children(array).into_iter().any(|element| {
        let literal: String = pieces(element, source)
            .iter()
            .filter(|piece| piece.kind == PieceKind::Literal)
            .map(|piece| piece.text.as_str())
            .collect();
        element_value(&literal).contains(ELEMENT_COMMA)
    });
    if has_comma {
        findings.report(ShellRule::Sc2054, array, text(array, source));
    }
}

fn element_value(literal: &str) -> &str {
    if !literal.starts_with(INDEX_OPEN) {
        return literal;
    }
    literal
        .split_once(INDEX_VALUE)
        .map_or(literal, |(_, value)| value)
}

pub fn is_append(assignment: Node<'_>, source: &[u8]) -> bool {
    children(assignment)
        .into_iter()
        .any(|child| !child.is_named() && text(child, source) == APPEND)
}

// SC2324: `checkPlusEqualsNumber`.
fn check_append_number(
    assignment: Node<'_>,
    context: &Context<'_, '_>,
    numeric_values: &NumericValues,
    findings: &mut Findings,
) {
    let source = context.source;
    if !is_append(assignment, source) {
        return;
    }
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD) else {
        return;
    };
    let is_number = unquoted_literal(&pieces(value, source)).is_some_and(|literal| {
        !literal.is_empty() && literal.chars().all(|character| character.is_ascii_digit())
    });
    let is_unmodified = !EXPANSIONS.contains(&value.kind()) || {
        let braced = braced_content(value, source);
        reference(&braced) == braced
    };
    if (is_number || (is_unmodified && numeric_values.contains_value(value)))
        && !numeric_values.assigns_to_integer(assignment)
    {
        findings.report(ShellRule::Sc2324, assignment, text(assignment, source));
    }
}

// SC2100: `checkWrongArithmeticAssignment`.
fn check_unexpanded_arithmetic(assignment: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let is_alone = assignment
        .parent()
        .is_none_or(|parent| ![COMMAND, DECLARATION, ASSIGNMENTS].contains(&parent.kind()));
    if !is_alone || assignment.child_by_field_name(NAME_FIELD).is_none() {
        return;
    }
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD) else {
        return;
    };
    let value_pieces = pieces(value, source);
    if !value_pieces
        .iter()
        .all(|piece| matches!(piece.kind, PieceKind::Literal | PieceKind::Glob))
    {
        return;
    }
    let raw: String = value_pieces.iter().map(|piece| piece.text.as_str()).collect();
    if let Some(variable) = arithmetic_operand(&raw)
        && flow(context).events.iter().any(|event| {
            matches!(event.kind, EventKind::Assignment | EventKind::Checked) && event.name == variable
        })
    {
        findings.report(ShellRule::Sc2100, value, &raw);
    }
}

// La expresión `^([_a-zA-Z][_a-zA-Z0-9]*)([+*-]).+$` de ShellCheck.
fn arithmetic_operand(raw: &str) -> Option<&str> {
    let (name, rest) = raw.split_at(raw.find(ARITHMETIC_SIGNS)?);
    let mut name_characters = name.chars();
    let valid_start = name_characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == NAME_EXTRA);
    let valid_name =
        valid_start && name_characters.all(|letter| letter.is_ascii_alphanumeric() || letter == NAME_EXTRA);
    (valid_name && rest.chars().count() > 1).then_some(name)
}
