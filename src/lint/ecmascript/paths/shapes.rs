use tree_sitter::Node;

use super::loops;
use super::roles::{
    ASSIGNMENT_PATTERNS, DEFAULTED_PARAMETERS, Place, SWITCH_CASES, children_with_fields, is_breakable,
};
use super::segments::Graph;
use super::state::State;

const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_statement";
const SWITCH_BODY: &str = "switch_body";
const SWITCH_CASE: &str = "switch_case";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const RIGHT_FIELD: &str = "right";
const LEFT_FIELD: &str = "left";
const INCREMENT_FIELD: &str = "increment";

// La condición de un loop, con su valor si es un literal, o cualquier otra parte.
#[derive(Clone, Copy)]
pub enum LoopPart {
    Test(Option<bool>),
    Other,
}

pub fn preprocess_loop(state: &mut State, graph: &mut Graph, place: Place<'_>, part: LoopPart) {
    let is = |field: &str| place.is(field);
    match (place.parent_kind(), part) {
        (WHILE, LoopPart::Test(constant)) => loops::make_while_test(state, graph, constant),
        (WHILE, LoopPart::Other) if is(BODY_FIELD) => loops::make_while_body(state, graph),
        (DO, LoopPart::Other) if is(BODY_FIELD) => loops::make_do_while_body(state, graph),
        (DO, LoopPart::Test(constant)) => loops::make_do_while_test(state, graph, constant),
        (FOR, LoopPart::Test(constant)) => loops::make_for_test(state, graph, constant),
        (FOR, LoopPart::Other) if is(INCREMENT_FIELD) => loops::make_for_update(state, graph),
        (FOR, LoopPart::Other) if is(BODY_FIELD) => loops::make_for_body(state, graph),
        (FOR_IN, _) if is(LEFT_FIELD) => loops::make_for_in_left(state, graph),
        (FOR_IN, _) if is(RIGHT_FIELD) => loops::make_for_in_right(state, graph),
        (FOR_IN, _) if is(BODY_FIELD) => loops::make_for_in_body(state, graph),
        (kind, _)
            if (ASSIGNMENT_PATTERNS.contains(&kind) && is(RIGHT_FIELD))
                || (DEFAULTED_PARAMETERS.contains(&kind) && is(VALUE_FIELD)) =>
        {
            state.push_fork(graph, false);
            state.fork_bypass_path(graph);
            state.fork_path(graph);
        }
        _ => {}
    }
}

pub fn is_pattern_with_default(node: Node<'_>) -> bool {
    ASSIGNMENT_PATTERNS.contains(&node.kind())
        || (DEFAULTED_PARAMETERS.contains(&node.kind()) && node.child_by_field_name(VALUE_FIELD).is_some())
}

pub fn labeled_body_is_breakable(labeled: Node<'_>) -> bool {
    labeled
        .child_by_field_name(BODY_FIELD)
        .is_some_and(|body| is_breakable(body.kind()))
}

pub fn has_case(switch: Node<'_>) -> bool {
    switch.child_by_field_name(BODY_FIELD).is_some_and(|body| {
        children_with_fields(body)
            .iter()
            .any(|(case, _)| case.kind() == SWITCH_CASE)
    })
}

pub fn is_first_case(body: Node<'_>, case: Node<'_>) -> bool {
    body.kind() == SWITCH_BODY
        && children_with_fields(body)
            .into_iter()
            .find(|(child, _)| SWITCH_CASES.contains(&child.kind()))
            .is_some_and(|(first, _)| first.id() == case.id())
}

pub fn case_statements(case: Node<'_>) -> Vec<Node<'_>> {
    children_with_fields(case)
        .into_iter()
        .filter(|(_, field)| *field != Some(VALUE_FIELD))
        .map(|(child, _)| child)
        .collect()
}
