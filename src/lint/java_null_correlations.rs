use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_null_conditions::without_cast;
use super::java_null_flow::{Analysis, BINARY, LEFT_FIELD, NOT_EQUAL, NULL, OPERATOR_FIELD, RIGHT_FIELD};
use super::java_null_keys::tracked_key;
use crate::syntax::descendants;

const TERNARY: &str = "ternary_expression";
const BOOLEAN_TYPE: &str = "boolean_type";
const ASSIGNMENT: &str = "assignment_expression";
const IDENTIFIER: &str = "identifier";
const CONDITION_FIELD: &str = "condition";
const ALTERNATIVE_FIELD: &str = "alternative";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";
const METHODS: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "lambda_expression",
];

// Lo que IntelliJ sabe de dos variables a la vez: `ws = b != null ? b.getWorkspace() : null` hace
// que `ws` no nulo implique `b` no nulo, y `boolean colapsado = ... && padre != null` guarda un
// chequeo que vale donde se pregunte por `colapsado` (si la local no se reasigna).
#[derive(Default)]
pub(super) struct Correlations {
    implied: HashMap<String, String>,
    conditions: HashMap<String, (usize, usize)>,
}

pub(super) fn note_assignment(analysis: &mut Analysis<'_, '_>, variable: &str, value: Node<'_>) {
    let correlations = &mut analysis.correlations;
    correlations.implied.remove(variable);
    correlations
        .implied
        .retain(|_, implied| implied.as_str() != variable);
    if let Some(implied) = guarded_by(analysis, without_cast(value)) {
        analysis.correlations.implied.insert(variable.to_owned(), implied);
    }
}

// `x != null ? valor : null`: el resultado no nulo implica `x` no nulo.
fn guarded_by(analysis: &Analysis<'_, '_>, value: Node<'_>) -> Option<String> {
    if value.kind() != TERNARY
        || value
            .child_by_field_name(ALTERNATIVE_FIELD)
            .is_none_or(|alternative| without_cast(alternative).kind() != NULL)
    {
        return None;
    }
    let condition = without_cast(value.child_by_field_name(CONDITION_FIELD)?);
    let is_not_null_check = condition.kind() == BINARY
        && condition
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| operator.kind() == NOT_EQUAL);
    if !is_not_null_check {
        return None;
    }
    let (left, right) = (
        condition.child_by_field_name(LEFT_FIELD)?,
        condition.child_by_field_name(RIGHT_FIELD)?,
    );
    let tested = match (left.kind(), right.kind()) {
        (_, NULL) => left,
        (NULL, _) => right,
        _ => return None,
    };
    tracked_key(analysis, without_cast(tested))
}

pub(super) fn implied_by<'analysis>(
    analysis: &'analysis Analysis<'_, '_>,
    variable: &str,
) -> Option<&'analysis String> {
    analysis.correlations.implied.get(variable)
}

// Una local `boolean` que nadie reasigna y se inicializa con una condición.
pub(super) fn note_condition(analysis: &mut Analysis<'_, '_>, declaration: Node<'_>, declarator: Node<'_>) {
    let is_boolean = declaration
        .child_by_field_name(TYPE_FIELD)
        .is_some_and(|written| written.kind() == BOOLEAN_TYPE);
    let (Some(name), Some(value)) = (
        declarator.child_by_field_name(NAME_FIELD),
        declarator.child_by_field_name(VALUE_FIELD),
    ) else {
        return;
    };
    let variable = text(name, analysis.source);
    if !is_boolean || is_reassigned(declaration, variable, analysis.source) {
        return;
    }
    analysis
        .correlations
        .conditions
        .insert(variable.to_owned(), (value.start_byte(), value.end_byte()));
}

// La condición que guarda la local que se pregunta, buscada en el método donde está la pregunta.
pub(super) fn stored_condition<'tree>(
    analysis: &Analysis<'_, '_>,
    asked: Node<'tree>,
) -> Option<Node<'tree>> {
    if asked.kind() != IDENTIFIER {
        return None;
    }
    let (start, end) = *analysis.correlations.conditions.get(analysis.text(asked))?;
    let method =
        std::iter::successors(asked.parent(), Node::parent).find(|node| METHODS.contains(&node.kind()))?;
    method
        .descendant_for_byte_range(start, end)
        .filter(|condition| condition.start_byte() == start && condition.end_byte() == end)
}

fn is_reassigned(declaration: Node<'_>, variable: &str, source: &[u8]) -> bool {
    let Some(method) =
        std::iter::successors(declaration.parent(), Node::parent).find(|node| METHODS.contains(&node.kind()))
    else {
        return true;
    };
    descendants(method).into_iter().any(|node| {
        node.kind() == ASSIGNMENT
            && node
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.kind() == IDENTIFIER && text(left, source) == variable)
    })
}
