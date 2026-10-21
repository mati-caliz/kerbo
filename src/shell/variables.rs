mod arithmetic;

pub use arithmetic::let_assignments;
mod declarations;

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::commands::Command;
use super::context::{Context, Findings, Position};
use super::environment::INTERNAL_VARIABLES;
use super::parameters::{
    index_references, is_variable_character, is_variable_name, modifier, offset_references, reference,
};
use super::syntax::{
    ASSIGNMENT, COMMAND, DECLARATION, EXPANSIONS, FOR, NAME_FIELD, SUBSCRIPT, VALUE_FIELD, VARIABLE_FIELD,
    children, text,
};
use super::words::{braced_content, literal_or};
use crate::rules::ShellRule;

const PRIVATE_PREFIX: char = '_';
const APPEND: &str = "+=";
const PROMPT_VARIABLES: &[&str] = &["PS1", "PS2", "PS3", "PS4", "PROMPT_COMMAND"];
const DOLLAR: char = '$';
const OPEN_BRACE: char = '{';
const NOT_LITERAL: &str = " ";
const PATTERN_LEAVES: &[&str] = &["regex", "extglob_pattern"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Assignment,
    Reference,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub name: String,
    pub position: Position,
}

#[derive(Debug, Default)]
pub struct Flow {
    pub associative: HashSet<String>,
    pub events: Vec<Event>,
}

impl Flow {
    pub fn assign(&mut self, name: impl Into<String>, position: Position) {
        self.events.push(Event {
            kind: EventKind::Assignment,
            name: name.into(),
            position,
        });
    }

    pub fn refer(&mut self, name: impl Into<String>, position: Position) {
        self.events.push(Event {
            kind: EventKind::Reference,
            name: name.into(),
            position,
        });
    }
}

pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let flow = flow(context.root, context.source, context.associative.clone());
    let referenced: HashSet<String> = flow
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Reference)
        .map(|event| {
            event
                .name
                .chars()
                .take_while(|character| is_variable_character(*character))
                .collect()
        })
        .chain(INTERNAL_VARIABLES.iter().map(|name| (*name).to_owned()))
        .collect();
    let mut last_assignment: HashMap<&str, Position> = HashMap::new();
    for event in &flow.events {
        if event.kind == EventKind::Assignment && is_variable_name(&event.name) {
            last_assignment.insert(event.name.as_str(), event.position);
        }
    }
    let mut unused: Vec<(&str, Position)> = last_assignment
        .into_iter()
        .filter(|(name, _)| !referenced.contains(*name) && !name.starts_with(PRIVATE_PREFIX))
        .collect();
    unused.sort_by_key(|(_, position)| position.byte);
    for (name, position) in unused {
        findings.report_at(ShellRule::Sc2034, position, name);
    }
}

pub fn flow(root: Node<'_>, source: &[u8], associative: HashSet<String>) -> Flow {
    let mut flow = Flow {
        events: Vec::new(),
        associative,
    };
    visit(root, source, &mut flow);
    flow
}

fn visit(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    if node.kind() == FOR
        && let Some(variable) = node.child_by_field_name(VARIABLE_FIELD)
    {
        flow.assign(text(variable, source), Position::of(node));
    }
    for child in children(node) {
        visit(child, source, flow);
    }
    references(node, source, flow);
    modifications(node, source, flow);
}

fn references(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    if EXPANSIONS.contains(&node.kind()) {
        let braced = braced_content(node, source);
        let position = Position::of(node);
        flow.refer(reference(&braced), position);
        for name in index_references(&braced)
            .into_iter()
            .chain(offset_references(&modifier(&braced)))
        {
            flow.refer(name, position);
        }
        return;
    }
    if node.kind() == ASSIGNMENT {
        assignment_references(node, source, flow);
    }
    if PATTERN_LEAVES.contains(&node.kind()) {
        for name in variables_in_literal(text(node, source)) {
            flow.refer(name, Position::of(node));
        }
    }
    arithmetic::references(node, source, flow);
    declarations::references(node, source, flow);
}

fn assignment_references(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    let Some(name) = assigned_name(node, source) else {
        return;
    };
    let position = Position::of(node);
    let appends = children(node)
        .into_iter()
        .any(|child| !child.is_named() && text(child, source) == APPEND);
    if appends {
        flow.refer(name.clone(), position);
    }
    if PROMPT_VARIABLES.contains(&name.as_str())
        && let Some(value) = node.child_by_field_name(VALUE_FIELD)
    {
        for referenced in variables_in_literal(&literal_or(value, source, NOT_LITERAL)) {
            flow.refer(referenced, position);
        }
    }
}

pub fn assigned_name(assignment: Node<'_>, source: &[u8]) -> Option<String> {
    let name = assignment.child_by_field_name(NAME_FIELD)?;
    let name = if name.kind() == SUBSCRIPT {
        name.child_by_field_name(NAME_FIELD)?
    } else {
        name
    };
    Some(text(name, source).to_owned())
}

fn modifications(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    match node.kind() {
        ASSIGNMENT if is_standalone_assignment(node) => {
            if let Some(name) = assigned_name(node, source) {
                flow.assign(name, Position::of(node));
            }
        }
        COMMAND | DECLARATION => {
            if let Some(command) = Command::of(node) {
                declarations::modifications(&command, source, flow);
            }
        }
        _ => arithmetic::modifications(node, source, flow),
    }
}

fn is_standalone_assignment(assignment: Node<'_>) -> bool {
    assignment
        .parent()
        .is_none_or(|parent| ![COMMAND, DECLARATION].contains(&parent.kind()))
}

pub fn variables_in_literal(text: &str) -> Vec<String> {
    text.split(DOLLAR)
        .skip(1)
        .map(|after| {
            after
                .strip_prefix(OPEN_BRACE)
                .unwrap_or(after)
                .chars()
                .take_while(|character| is_variable_character(*character))
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}
