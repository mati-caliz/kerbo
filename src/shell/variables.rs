mod arithmetic;

pub use arithmetic::let_assignments;
mod checks;
mod declarations;
mod subshells;
mod unassigned;

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::commands::Command;
use super::context::{Context, Findings, Position};
use super::environment::INTERNAL_VARIABLES;
use super::flow::has_lastpipe;
use super::named_descriptor::named_descriptor;
use super::parameters::{
    index_references, is_variable_character, is_variable_name, modifier, offset_references, reference,
};
use super::pattern_scan::{SegmentKind, segments};
use super::syntax::{
    ASSIGNMENT, BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, COMMENT, DECLARATION, DESTINATION_FIELD,
    EXPANSION, EXPANSIONS, FILE_REDIRECT, FOR, NAME_FIELD, PIPELINE, REDIRECTED, SUBSCRIPT, SUBSHELL, TEST,
    VALUE_FIELD, VARIABLE_FIELD, children, is_backgrounded, named_children, text,
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
const ASSIGNING_MODIFIERS: &[&str] = &["=", ":="];
const INDEX_START: char = '[';
const INDEX_END: char = ']';
const CLOSE_DESCRIPTOR: &str = "-";
const CLOSING_OPERATORS: &[&str] = &[">&-", "<&-"];
const PREFIXES: &[char] = &['#', '!'];
const GUARD_OPERATORS: &[char] = &['-', '?'];
const COLON: char = ':';
const DOUBLE_QUOTE_WIDTH: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Assignment,
    // `[ -n "$x" ]` o `[ -v x ]`: ShellCheck lo cuenta como asignado, para no avisar que no lo está.
    Checked,
    Reference,
    ScopeStart,
    ScopeEnd,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub name: String,
    pub position: Position,
    // `isTrueAssignmentSource`: `declare x` sin valor no le cambia el valor a `x`.
    pub is_true_source: bool,
    // `${x:-y}` o `${x:?}`, que ya contemplan que `x` no esté asignada.
    pub is_guarded: bool,
}

#[derive(Debug, Default)]
pub struct Flow {
    pub associative: HashSet<String>,
    pub events: Vec<Event>,
    pub has_lastpipe: bool,
}

impl Flow {
    fn push(&mut self, kind: EventKind, name: impl Into<String>, position: Position) {
        self.events.push(Event {
            kind,
            name: name.into(),
            position,
            is_true_source: true,
            is_guarded: false,
        });
    }

    pub fn assign(&mut self, name: impl Into<String>, position: Position) {
        self.push(EventKind::Assignment, name, position);
    }

    pub fn declare(&mut self, name: impl Into<String>, position: Position) {
        self.push(EventKind::Assignment, name, position);
        if let Some(event) = self.events.last_mut() {
            event.is_true_source = false;
        }
    }

    pub fn refer(&mut self, name: impl Into<String>, position: Position) {
        self.push(EventKind::Reference, name, position);
    }

    fn refer_braced(&mut self, braced: &str, position: Position) {
        self.push(EventKind::Reference, reference(braced), position);
        if let Some(event) = self.events.last_mut() {
            event.is_guarded = is_guarded(braced);
        }
    }
}

// `isGuarded`: después del nombre viene un índice opcional, un `:` opcional y un `-` o un `?`.
fn is_guarded(braced: &str) -> bool {
    let rest = braced
        .trim_start_matches(PREFIXES)
        .trim_start_matches(is_variable_character);
    let is_guard = |text: &str| {
        text.strip_prefix(COLON)
            .unwrap_or(text)
            .starts_with(GUARD_OPERATORS)
    };
    is_guard(rest)
        || (rest.starts_with(INDEX_START)
            && rest
                .match_indices(INDEX_END)
                .any(|(index, _)| is_guard(rest.get(index.saturating_add(1)..).unwrap_or_default())))
}

pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let flow = flow(context);
    subshells::check(&flow.events, findings);
    unassigned::check(&flow.events, context, findings);
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

pub fn flow(context: &Context<'_, '_>) -> Flow {
    let mut flow = Flow {
        events: Vec::new(),
        associative: context.associative.clone(),
        has_lastpipe: has_lastpipe(context),
    };
    visit(context.root, context.source, &mut flow);
    flow
}

// `leadType` de ShellCheck: lo que corre en un subshell. Un pipeline de varios comandos corre cada
// uno en el suyo; con `lastpipe`, el último no.
fn opens_subshell(node: Node<'_>, source: &[u8], flow: &Flow) -> bool {
    if [COMMAND_SUBSTITUTION, SUBSHELL].contains(&node.kind()) || is_backgrounded(node, source) {
        return true;
    }
    let Some(pipeline) = node
        .parent()
        .filter(|parent| parent.kind() == PIPELINE && node.is_named() && node.kind() != COMMENT)
    else {
        return false;
    };
    let is_last = named_children(pipeline)
        .last()
        .is_some_and(|last| last.id() == node.id());
    !(flow.has_lastpipe && is_last)
}

// Como en ShellCheck, las redirecciones se recorren antes que el comando.
fn visiting_order(node: Node<'_>) -> Vec<Node<'_>> {
    let mut ordered = children(node);
    if node.kind() == REDIRECTED {
        let body = node.child_by_field_name(BODY_FIELD).map(|body| body.id());
        ordered.sort_by_key(|child| Some(child.id()) == body);
    }
    ordered
}

fn visit(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    let is_subshell = opens_subshell(node, source, flow);
    if is_subshell {
        flow.push(EventKind::ScopeStart, "", Position::of(node));
    }
    if node.kind() == FOR
        && let Some(variable) = node.child_by_field_name(VARIABLE_FIELD)
    {
        flow.assign(text(variable, source), Position::of(node));
    }
    for child in visiting_order(node) {
        visit(child, source, flow);
    }
    references(node, source, flow);
    modifications(node, source, flow);
    if is_subshell {
        flow.push(EventKind::ScopeEnd, "", Position::of(node));
    }
}

fn references(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    if EXPANSIONS.contains(&node.kind()) {
        let braced = braced_content(node, source);
        let position = Position::of(node);
        flow.refer_braced(&braced, position);
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
        pattern_references(text(node, source), Position::of(node), flow);
    }
    if node.kind() == FILE_REDIRECT
        && closes_descriptor(node, source)
        && let Some((name, position)) = named_descriptor(node, source)
    {
        flow.refer(name, position);
    }
    arithmetic::references(node, source, flow);
    declarations::references(node, source, flow);
}

// Las expansiones de una hoja `regex`: lo de adentro de comillas dobles cuenta, lo de simples no.
fn pattern_references(pattern: &str, position: Position, flow: &mut Flow) {
    for segment in segments(pattern) {
        let inside = pattern.get(segment.start..segment.end).unwrap_or_default();
        let at = position.shifted(segment.start);
        match segment.kind {
            SegmentKind::Expansion => {
                let braced = inside
                    .strip_prefix(DOLLAR)
                    .map(|rest| {
                        rest.strip_prefix(OPEN_BRACE)
                            .map_or(rest, |open| open.trim_end_matches('}'))
                    })
                    .unwrap_or_default();
                flow.refer_braced(braced, at);
            }
            SegmentKind::DoubleQuoted => {
                let unquoted = inside.trim_matches('"');
                pattern_references(unquoted, at.shifted(DOUBLE_QUOTE_WIDTH), flow);
            }
            SegmentKind::Plain | SegmentKind::SingleQuoted | SegmentKind::Substitution => {}
        }
    }
}

fn closes_descriptor(redirect: Node<'_>, source: &[u8]) -> bool {
    redirect
        .child_by_field_name(DESTINATION_FIELD)
        .is_some_and(|destination| text(destination, source) == CLOSE_DESCRIPTOR)
        || children(redirect)
            .into_iter()
            .any(|child| !child.is_named() && CLOSING_OPERATORS.contains(&text(child, source)))
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
        EXPANSION => {
            let braced = braced_content(node, source);
            let assigned = modifier(&braced);
            if ASSIGNING_MODIFIERS
                .iter()
                .any(|operator| assigned.starts_with(operator))
            {
                flow.assign(reference(&braced), Position::of(node));
            }
        }
        TEST => checks::checked_variables(node, source, flow),
        FILE_REDIRECT if !closes_descriptor(node, source) => {
            if let Some((name, position)) = named_descriptor(node, source) {
                flow.assign(name, position);
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
