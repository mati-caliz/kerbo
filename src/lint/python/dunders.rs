use std::cmp::Ordering;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, AUGMENTED_ASSIGNMENT, CALL, CLASS, COMMENT,
    CONCATENATED_STRING, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, LIST, OBJECT_FIELD, RIGHT_FIELD,
    SET, TUPLE, is_bytes, is_f_string, is_string, named, text, unparenthesized,
};
use super::escapes::text_value;
use super::natural::{isort_compare, natural_compare};
use crate::rules::PythonRule;

const ALL: &str = "__all__";
const SLOTS: &str = "__slots__";
const EXTEND: &str = "extend";
const ADD: &str = "+=";
const OPERATOR_FIELD: &str = "operator";
const EXPRESSION_LIST: &str = "expression_list";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";

// RUF022 (`__all__` sin ordenar, en el módulo) y RUF023 (`__slots__` sin ordenar, en una clase).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter_map(|node| match node.kind() {
            ASSIGNMENT => assignment_report(*node, source),
            AUGMENTED_ASSIGNMENT => augmented_report(*node, source),
            CALL => extend_report(*node, source),
            _ => None,
        })
        .collect()
}

fn assignment_report(assignment: Node<'_>, source: &[u8]) -> Option<Report> {
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let value = assignment.child_by_field_name(RIGHT_FIELD)?;
    let chained = value.kind() == ASSIGNMENT
        || assignment
            .parent()
            .is_some_and(|parent| parent.kind() == ASSIGNMENT);
    if target.kind() != IDENTIFIER || chained {
        return None;
    }
    match (
        text(target, source),
        enclosing_definition(assignment).map(|owner| owner.kind()),
    ) {
        (ALL, None) => unsorted_all(value, source),
        (SLOTS, Some(CLASS)) => unsorted_slots(value, source),
        _ => None,
    }
}

fn augmented_report(assignment: Node<'_>, source: &[u8]) -> Option<Report> {
    let is_addition = assignment
        .child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| text(operator, source) == ADD);
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let names_all = target.kind() == IDENTIFIER && text(target, source) == ALL;
    if !is_addition || !names_all || enclosing_definition(assignment).is_some() {
        return None;
    }
    unsorted_all(assignment.child_by_field_name(RIGHT_FIELD)?, source)
}

// `__all__.extend([...])` en el módulo.
fn extend_report(call: Node<'_>, source: &[u8]) -> Option<Report> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let is_extend = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == EXTEND);
    let on_all = function
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == ALL);
    let arguments: Vec<Node<'_>> = named(call.child_by_field_name(ARGUMENTS_FIELD)?)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    let [argument] = arguments.as_slice() else {
        return None;
    };
    if !is_extend || !on_all || enclosing_definition(call).is_some() {
        return None;
    }
    unsorted_all(*argument, source)
}

fn unsorted_all(value: Node<'_>, source: &[u8]) -> Option<Report> {
    let value = unparenthesized(value);
    if !matches!(value.kind(), LIST | TUPLE | EXPRESSION_LIST) {
        return None;
    }
    is_unsorted(&elements(value), source, isort_compare).then(|| Report::at(PythonRule::RUF022, value, ""))
}

fn unsorted_slots(value: Node<'_>, source: &[u8]) -> Option<Report> {
    let value = unparenthesized(value);
    let items = match value.kind() {
        LIST | TUPLE | EXPRESSION_LIST | SET => elements(value),
        DICTIONARY => {
            let entries = elements(value);
            if entries.iter().any(|entry| entry.kind() != PAIR) {
                return None;
            }
            entries
                .iter()
                .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
                .collect()
        }
        _ => return None,
    };
    is_unsorted(&items, source, natural_compare).then(|| Report::at(PythonRule::RUF023, value, ""))
}

fn elements(collection: Node<'_>) -> Vec<Node<'_>> {
    named(collection)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect()
}

// `SortClassification::of_elements`: todos strings literales y alguno fuera de orden.
fn is_unsorted(elements: &[Node<'_>], source: &[u8], compare: fn(&str, &str) -> Ordering) -> bool {
    let values: Option<Vec<String>> = elements
        .iter()
        .map(|element| string_literal(*element, source))
        .collect();
    values.is_some_and(|values| {
        values
            .windows(2)
            .any(|pair| matches!(pair, [previous, next] if compare(next, previous) == Ordering::Less))
    })
}

fn string_literal(element: Node<'_>, source: &[u8]) -> Option<String> {
    let element = unparenthesized(element);
    let parts = if element.kind() == CONCATENATED_STRING {
        named(element)
    } else {
        vec![element]
    };
    let plain = is_string(element)
        && parts
            .iter()
            .all(|part| !is_bytes(*part, source) && !is_f_string(*part, source));
    if plain { text_value(element, source) } else { None }
}

// La función o clase que encierra al nodo; `None` en el módulo.
fn enclosing_definition(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS))
}
