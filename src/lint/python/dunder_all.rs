use std::collections::HashSet;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, AUGMENTED_ASSIGNMENT, CALL, CLASS, COMMENT,
    FUNCTION, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, LEFT_FIELD, LIST, OBJECT_FIELD,
    RIGHT_FIELD, SET, STRING, TUPLE, named, text, unparenthesized,
};
use super::escapes::text_value;
use crate::rules::PythonRule;

const ALL: &str = "__all__";
const EXTEND: &str = "extend";
const ADDITION: &str = "+=";
const BINARY: &str = "binary_operator";
const NAMED_EXPRESSION: &str = "named_expression";
const SEQUENCE_BUILDERS: [&str; 2] = ["list", "tuple"];
const OPERATOR_FIELD: &str = "operator";
const VALUE_FIELD: &str = "value";
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];

// RUF068 (un nombre repetido en `__all__`) y PLE0604 (algo que no es un string en `__all__`), sólo
// en el alcance del módulo.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        if !matches!(node.kind(), ASSIGNMENT | AUGMENTED_ASSIGNMENT | CALL) || !in_module_scope(*node) {
            continue;
        }
        if node.kind() == CALL {
            found.extend(extended_duplicates(*node, source));
            continue;
        }
        let Some(target) = node
            .child_by_field_name(LEFT_FIELD)
            .filter(|target| target.kind() == IDENTIFIER && text(*target, source) == ALL)
        else {
            continue;
        };
        let Some(value) = node.child_by_field_name(RIGHT_FIELD) else {
            continue;
        };
        if value.kind() == ASSIGNMENT || node.parent().is_some_and(|parent| parent.kind() == ASSIGNMENT) {
            continue;
        }
        let adds = node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == ADDITION);
        if node.kind() == ASSIGNMENT || adds {
            found.extend(duplicates(value, source));
        }
        if has_invalid_object(value, source) {
            found.push(Report::at(PythonRule::PLE0604, target, ""));
        }
    }
    found
}

fn in_module_scope(node: Node<'_>) -> bool {
    !std::iter::successors(node.parent(), Node::parent).any(|ancestor| SCOPES.contains(&ancestor.kind()))
}

fn elements(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn string_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    matches!(node.kind(), STRING | "concatenated_string")
        .then(|| text_value(node, source))
        .flatten()
}

// `__all__.extend([...])` con un único argumento.
fn extended_duplicates(call: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(function) = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)
    else {
        return Vec::new();
    };
    let extends = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == EXTEND)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == ALL);
    let arguments = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(elements)
        .unwrap_or_default();
    match arguments.as_slice() {
        [only] if extends && only.kind() != KEYWORD_ARGUMENT => duplicates(*only, source),
        _ => Vec::new(),
    }
}

fn duplicates(value: Node<'_>, source: &[u8]) -> Vec<Report> {
    if !matches!(value.kind(), LIST | TUPLE) {
        return Vec::new();
    }
    let items = elements(value);
    if items.len() <= 1 {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter_map(|item| {
            let name = string_value(item, source)?;
            (!seen.insert(name)).then(|| Report::at(PythonRule::RUF068, item, ""))
        })
        .collect()
}

// Lo que el `extract_dunder_all_names` de ruff marca como `INVALID_OBJECT`: un elemento que no es un
// string literal en las listas o tuplas que arman `__all__`, también sumadas con `+`.
fn has_invalid_object(value: Node<'_>, source: &[u8]) -> bool {
    let value = unparenthesized(value);
    if value.kind() == BINARY {
        return [LEFT_FIELD, RIGHT_FIELD].into_iter().any(|side| {
            value
                .child_by_field_name(side)
                .is_some_and(|operand| has_invalid_object(operand, source))
        });
    }
    sequence_elements(value, source)
        .iter()
        .any(|element| string_value(*element, source).is_none())
}

fn sequence_elements<'tree>(value: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    match value.kind() {
        LIST | TUPLE => elements(value),
        NAMED_EXPRESSION => value
            .child_by_field_name(VALUE_FIELD)
            .map(|inner| sequence_elements(unparenthesized(inner), source))
            .unwrap_or_default(),
        CALL => {
            let builds = value.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
                function.kind() == IDENTIFIER && SEQUENCE_BUILDERS.contains(&text(function, source))
            });
            let arguments = value
                .child_by_field_name(ARGUMENTS_FIELD)
                .map(elements)
                .unwrap_or_default();
            match arguments.as_slice() {
                [only] if builds && matches!(only.kind(), LIST | SET | TUPLE) => elements(*only),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}
