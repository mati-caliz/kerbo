use tree_sitter::Node;

use super::java::NAME_FIELD;
use super::java_chains::argument_count;
use super::java_null_flow::{
    ARGUMENTS_FIELD, Analysis, COMMENTS, Dereference, IDENTIFIER, INVOCATION, NullFlow, OBJECT_FIELD, State,
};
use super::java_null_inference::{CallTarget, NullArgument, call_target};
use crate::syntax::{named_children, one_based};

const FIELD_ACCESS: &str = "field_access";
const ARRAY_ACCESS: &str = "array_access";
const FIELD_FIELD: &str = "field";
const ARRAY_FIELD: &str = "array";

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
    let passed: Vec<(usize, Node<'_>, usize)> = arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| argument.kind() == IDENTIFIER)
        .filter_map(|(index, argument)| Some((index, *argument, *state.get(analysis.text(*argument))?)))
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
    for (index, argument, origin) in passed {
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
        };
        let Some(flow) = analysis.flows.get_mut(origin) else {
            continue;
        };
        flow.arguments.push(used.clone());
        let mut passed_before = flow.passed_before.clone();
        passed_before.push(used);
        let after = NullFlow {
            origin: flow.origin.clone(),
            dereferences: Vec::new(),
            arguments: Vec::new(),
            passed_before,
        };
        state.insert(analysis.text(argument).to_owned(), analysis.flows.len());
        analysis.flows.push(after);
    }
    state
}

pub(super) fn dereference(analysis: &mut Analysis<'_, '_>, node: Node<'_>, mut state: State) -> State {
    let (receiver, used) = match node.kind() {
        INVOCATION | FIELD_ACCESS => (
            node.child_by_field_name(OBJECT_FIELD),
            node.child_by_field_name(if node.kind() == INVOCATION {
                NAME_FIELD
            } else {
                FIELD_FIELD
            }),
        ),
        ARRAY_ACCESS => (
            node.child_by_field_name(ARRAY_FIELD),
            node.child_by_field_name(ARRAY_FIELD),
        ),
        _ => (None, None),
    };
    let (Some(receiver), Some(used)) = (receiver.filter(|receiver| receiver.kind() == IDENTIFIER), used)
    else {
        return state;
    };
    if let Some(origin) = state.remove(analysis.text(receiver)) {
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
