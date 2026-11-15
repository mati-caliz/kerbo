use std::collections::hash_map::Entry;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_null_flow::{Analysis, NullFlow, NullOrigin, State};
use crate::syntax::{descendants, named_children};

const NULL: &str = "null_literal";
const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const BLOCK: &str = "block";
const ASSIGNMENT: &str = "assignment_expression";
const IDENTIFIER: &str = "identifier";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const BODY_FIELD: &str = "body";

// Las locales que arrancan en `null`, siguen así al entrar al `try` y se asignan adentro: en un
// `catch` pueden seguir en null, porque la excepción pudo saltar antes de la asignación. Fuera de ese
// camino un `null` inicial no se sigue: IntelliJ separa los estados por las condiciones que
// correlacionan la local con otra variable (`hp2` asignado sólo si `!badDefaultPort`), y kerbo no.
// Los flujos del `null` inicial, creados antes de recorrer el `try`: al juntar el `catch` con el
// cuerpo queda el flujo más viejo, y tiene que ser éste.
pub(super) fn failure_nulls(
    analysis: &mut Analysis<'_, '_>,
    try_statement: Node<'_>,
) -> Vec<(String, usize)> {
    null_after_failure(try_statement, analysis.source)
        .into_iter()
        .map(|local| {
            analysis.flows.push(NullFlow::new(NullOrigin::Literal));
            (local, analysis.flows.len() - 1)
        })
        .collect()
}

pub(super) fn catch_entry(state: &State, failure_nulls: &[(String, usize)]) -> State {
    let mut entry = state.clone();
    for (local, flow) in failure_nulls {
        if let Entry::Vacant(vacant) = entry.entry(local.clone()) {
            vacant.insert(*flow);
        }
    }
    entry
}

fn null_after_failure(try_statement: Node<'_>, source: &[u8]) -> Vec<String> {
    let Some(body) = try_statement.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    let start = try_statement.start_byte();
    null_declared_before(try_statement, source)
        .into_iter()
        .filter(|(name, declared_at)| {
            !assigned_between(try_statement, name, (*declared_at, start), source)
                && !assignments_to(body, name, source).is_empty()
        })
        .map(|(name, _)| name)
        .collect()
}

// Las locales declaradas con `= null` en los bloques que encierran al `try`, antes de él, con dónde
// termina su declaración.
fn null_declared_before(try_statement: Node<'_>, source: &[u8]) -> Vec<(String, usize)> {
    let start = try_statement.start_byte();
    std::iter::successors(try_statement.parent(), Node::parent)
        .filter(|ancestor| ancestor.kind() == BLOCK)
        .flat_map(named_children)
        .filter(|statement| statement.kind() == LOCAL && statement.end_byte() <= start)
        .flat_map(named_children)
        .filter(|declarator| declarator.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| {
            declarator
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| value.kind() == NULL)
        })
        .filter_map(|declarator| {
            let name = declarator.child_by_field_name(NAME_FIELD)?;
            Some((text(name, source).to_owned(), declarator.end_byte()))
        })
        .collect()
}

fn assigned_between(try_statement: Node<'_>, name: &str, (from, to): (usize, usize), source: &[u8]) -> bool {
    let Some(method_body) = std::iter::successors(try_statement.parent(), Node::parent)
        .filter(|ancestor| ancestor.kind() == BLOCK)
        .last()
    else {
        return false;
    };
    assignments_to(method_body, name, source)
        .into_iter()
        .any(|assignment| assignment.start_byte() >= from && assignment.start_byte() < to)
}

fn assignments_to<'tree>(scope: Node<'tree>, name: &str, source: &[u8]) -> Vec<Node<'tree>> {
    descendants(scope)
        .into_iter()
        .filter(|node| node.kind() == ASSIGNMENT)
        .filter(|assignment| {
            assignment
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.kind() == IDENTIFIER && text(left, source) == name)
        })
        .collect()
}
