use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BODY_FIELD, CALL, CLASS, DECORATED, FUNCTION, FUNCTION_FIELD, VALUE_FIELD, named,
};
use crate::rules::PythonRule;

const ASYNC: &str = "async";
const AWAIT: &str = "await";
const YIELD: &str = "yield";
const WITH_CLAUSE: &str = "with_clause";
const AS_PATTERN: &str = "as_pattern";
const ASYNC_STATEMENTS: [&str; 3] = ["with_statement", "for_statement", "for_in_clause"];
const DEFINITIONS: [&str; 3] = [FUNCTION, CLASS, DECORATED];
const TIMEOUTS: [&str; 10] = [
    "asyncio.timeout",
    "asyncio.timeout_at",
    "anyio.move_on_after",
    "anyio.fail_after",
    "anyio.CancelScope",
    "trio.CancelScope",
    "trio.fail_after",
    "trio.fail_at",
    "trio.move_on_after",
    "trio.move_on_at",
];

// ASYNC100: un `with` de timeout sin ningún `await` adentro, que nunca puede cortar nada. Un `yield`
// cuenta como punto de corte, porque el corte puede pasar en quien llama.
pub fn timeout_without_checkpoint(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Option<Report> {
    let contexts: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|child| child.kind() == WITH_CLAUSE)
        .flat_map(named)
        .filter_map(|item| item.child_by_field_name(VALUE_FIELD))
        .map(|value| {
            if value.kind() == AS_PATTERN {
                named(value).first().copied().unwrap_or(value)
            } else {
                value
            }
        })
        .collect();
    let (position, method) = contexts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, context)| {
            let name = (context.kind() == CALL)
                .then(|| context.child_by_field_name(FUNCTION_FIELD))
                .flatten()
                .and_then(|function| qualified_name(model, function, source))?;
            TIMEOUTS.contains(&name.as_str()).then_some((position, name))
        })?;
    let is_async = starts_with_async(statement, source);
    if is_async && position.saturating_add(1) < contexts.len() {
        return None;
    }
    let body = statement.child_by_field_name(BODY_FIELD)?;
    if contains_yield(body) || contains_checkpoint(body, source) {
        return None;
    }
    Some(Report::at(PythonRule::ASYNC100, statement, &method))
}

fn starts_with_async(node: Node<'_>, source: &[u8]) -> bool {
    node.child(0)
        .and_then(|first| first.utf8_text(source).ok())
        .is_some_and(|first| first == ASYNC)
}

fn contains_yield(node: Node<'_>) -> bool {
    named(node)
        .into_iter()
        .any(|child| child.kind() == YIELD || contains_yield(child))
}

fn contains_checkpoint(node: Node<'_>, source: &[u8]) -> bool {
    named(node).into_iter().any(|child| {
        if DEFINITIONS.contains(&child.kind()) {
            return false;
        }
        (child.kind() == AWAIT && child.is_named())
            || (ASYNC_STATEMENTS.contains(&child.kind()) && starts_with_async(child, source))
            || contains_checkpoint(child, source)
    })
}
