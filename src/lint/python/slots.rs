use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, BLOCK, BODY_FIELD, CLASS, EXPRESSION_STATEMENT, IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, STRING,
    is_bytes, is_string, named, string_prefix, text, unparenthesized,
};
use crate::rules::PythonRule;

const SLOTS: &str = "__slots__";
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];

// PLC0205: `__slots__ = "nombre"` en el cuerpo de una clase, que declara un slot por letra en vez de
// uno con ese nombre.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == CLASS)
        .filter_map(|class| class.child_by_field_name(BODY_FIELD))
        .filter(|body| body.kind() == BLOCK)
        .flat_map(named)
        .filter(|statement| statement.kind() == EXPRESSION_STATEMENT)
        .flat_map(|statement| single_string_slots(statement, source))
        .collect()
}

fn single_string_slots(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(assignment) = named(statement)
        .into_iter()
        .find(|child| child.kind() == ASSIGNMENT)
    else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    let mut current = assignment;
    let value = loop {
        targets.extend(current.child_by_field_name(LEFT_FIELD));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(right) if right.kind() == ASSIGNMENT => current = right,
            right => break right,
        }
    };
    if !value.is_some_and(|value| is_text_string(unparenthesized(value), source)) {
        return Vec::new();
    }
    targets
        .into_iter()
        .filter(|target| target.kind() == IDENTIFIER && text(*target, source) == SLOTS)
        .map(|_| Report::at(PythonRule::PLC0205, statement, ""))
        .collect()
}

// Un string o una f-string, pero no bytes ni una t-string.
fn is_text_string(value: Node<'_>, source: &[u8]) -> bool {
    let parts = if value.kind() == STRING {
        vec![value]
    } else {
        named(value)
    };
    is_string(value)
        && !parts
            .iter()
            .any(|part| is_bytes(*part, source) || string_prefix(*part, source).contains(TEMPLATE_PREFIXES))
}
