use tree_sitter::Node;

use super::java::NAME_FIELD;
use super::java_chains::argument_count;
use super::java_null_flow::{
    ARGUMENTS_FIELD, Analysis, COMMENTS, Dereference, INVOCATION, NullFlow, OBJECT_FIELD, State,
};
use super::java_null_inference::{CallTarget, NullArgument, call_target};
use super::java_null_keys::{forget, spellings, tracked_key};
use super::java_receiver_arguments::receiver_arguments;
use crate::syntax::{named_children, one_based};

const FIELD_ACCESS: &str = "field_access";
const ARRAY_ACCESS: &str = "array_access";
const FIELD_FIELD: &str = "field";
const ARRAY_FIELD: &str = "array";
const CAST: &str = "cast_expression";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];

// Las variables posiblemente null que la llamada recibe como argumento. Siguen en el estado, en
// un flujo nuevo que recuerda la llamada: si el método no las desreferencia, el uso que viene
// después es el que cuenta.
pub(super) fn passed(analysis: &mut Analysis<'_, '_>, call: Node<'_>, mut state: State) -> State {
    let arguments: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .collect();
    let passed: Vec<(usize, Node<'_>, String, usize)> = arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            let key = tracked_key(analysis, *argument)?;
            let origin = *state.get(&key)?;
            Some((index, *argument, key, origin))
        })
        .collect();
    if passed.is_empty() {
        return state;
    }
    let Some(CallTarget {
        name,
        is_constructor,
        receiver,
    }) = call_target(call, analysis.source, analysis.imported, analysis.variables)
    else {
        return state;
    };
    let arity = argument_count(call);
    let receiver_arguments = receiver_arguments(call, analysis.source);
    for (index, argument, key, origin) in passed {
        let start = argument.start_position();
        let used = NullArgument {
            name: name.clone(),
            is_constructor,
            arity,
            receiver: receiver.clone(),
            index,
            line: one_based(start.row),
            column: one_based(start.column),
            cast: None,
            exact: false,
            receiver_arguments: receiver_arguments.clone(),
        };
        let Some(flow) = analysis.flows.get_mut(origin) else {
            continue;
        };
        flow.arguments.push(used.clone());
        let mut passed_before = flow.passed_before.clone();
        passed_before.push(used);
        let after = NullFlow {
            passed_before,
            ..NullFlow::new(flow.origin.clone())
        };
        for spelling in spellings(analysis, key) {
            state.insert(spelling, analysis.flows.len());
        }
        analysis.flows.push(after);
    }
    state
}

// IntelliJ marca la desreferencia de un campo en el calificador: en `this.x.y`, en `x`.
fn qualifier_name(object: Node<'_>) -> Node<'_> {
    object
        .child_by_field_name(FIELD_FIELD)
        .filter(|_| object.kind() == FIELD_ACCESS)
        .unwrap_or(object)
}

pub(super) fn dereference(analysis: &mut Analysis<'_, '_>, node: Node<'_>, mut state: State) -> State {
    let (receiver, used) = match node.kind() {
        INVOCATION => (
            node.child_by_field_name(OBJECT_FIELD),
            node.child_by_field_name(NAME_FIELD),
        ),
        FIELD_ACCESS => {
            let object = node.child_by_field_name(OBJECT_FIELD);
            (object, object.map(qualifier_name))
        }
        ARRAY_ACCESS => (
            node.child_by_field_name(ARRAY_FIELD),
            node.child_by_field_name(ARRAY_FIELD),
        ),
        CAST if casts_to_primitive(node) => (
            node.child_by_field_name(VALUE_FIELD),
            node.child_by_field_name(VALUE_FIELD),
        ),
        _ => (None, None),
    };
    let (Some(receiver), Some(used)) = (
        receiver.and_then(|receiver| tracked_key(analysis, receiver)),
        used,
    ) else {
        return state;
    };
    if let Some(origin) = forget(&mut state, &receiver) {
        let start = used.start_position();
        let name = analysis.text(used).to_owned();
        if let Some(flow) = analysis.flows.get_mut(origin) {
            flow.dereferences.push(Dereference {
                name,
                line: one_based(start.row),
                column: one_based(start.column),
            });
        }
    }
    state
}

// `(int) valor` desempaqueta el valor: si es null, falla como una desreferencia.
fn casts_to_primitive(cast: Node<'_>) -> bool {
    cast.child_by_field_name(TYPE_FIELD)
        .is_some_and(|written| PRIMITIVE_TYPES.contains(&written.kind()))
}
