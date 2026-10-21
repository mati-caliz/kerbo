use tree_sitter::Node;

use super::Flow;
use crate::shell::context::Position;
use crate::shell::parameters::{is_variable_character, variable_names};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ASSIGNMENT, BINARY, BODY_FIELD, INDEX_FIELD, LEFT_FIELD,
    NAME_FIELD, OPERATOR_FIELD, PARENTHESIZED, POSTFIX, SUBSCRIPT, TERNARY, UNARY, VARIABLE_NAME, WORD,
    children, is_arithmetic_command, text,
};

const ASSIGNMENT_OPERATORS: &[&str] = &["=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|="];
const PLAIN_ASSIGNMENT: &str = "=";
const STEPS: &[&str] = &["++", "--"];
const EXPRESSION_KINDS: &[&str] = &[BINARY, UNARY, POSTFIX, PARENTHESIZED, TERNARY];
const EQUALITY: &str = "==";
const DOLLAR: char = '$';
const UNDERSCORE: char = '_';

pub fn references(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    match node.kind() {
        VARIABLE_NAME | WORD
            if is_arithmetic_operand(node, source) && !is_plain_assignment_target(node, source) =>
        {
            for name in variable_names(text(node, source)) {
                flow.refer(name, Position::of(node));
            }
        }
        SUBSCRIPT => subscript_references(node, source, flow),
        _ => {}
    }
}

fn subscript_references(subscript: Node<'_>, source: &[u8], flow: &mut Flow) {
    let in_arithmetic = is_arithmetic_operand(subscript, source);
    let assigned = is_assignment_name(subscript);
    if !in_arithmetic && !assigned {
        return;
    }
    if in_arithmetic
        && !is_plain_assignment_target(subscript, source)
        && let Some(name) = subscript.child_by_field_name(NAME_FIELD)
    {
        flow.refer(text(name, source), Position::of(subscript));
    }
    let associative = subscript
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| flow.associative.contains(text(name, source)));
    if let Some(index) = subscript.child_by_field_name(INDEX_FIELD)
        && [WORD, VARIABLE_NAME].contains(&index.kind())
        && !(assigned && associative)
    {
        for name in variable_names(text(index, source)) {
            flow.refer(name, Position::of(index));
        }
    }
}

pub fn modifications(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    if !is_arithmetic_operand(node, source) && !is_arithmetic_root(node, source) {
        return;
    }
    match node.kind() {
        BINARY => {
            let operator = node.child_by_field_name(OPERATOR_FIELD);
            let target = node
                .child_by_field_name(LEFT_FIELD)
                .and_then(|left| target_name(left, source));
            if let (Some(operator), Some(name)) = (operator, target)
                && ASSIGNMENT_OPERATORS.contains(&text(operator, source))
            {
                flow.assign(name, Position::of(operator));
            }
        }
        POSTFIX | UNARY => {
            let steps = node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| STEPS.contains(&text(operator, source)));
            let operand = children(node).into_iter().find(Node::is_named);
            if steps
                && let Some(operand) = operand
                && let Some(name) = target_name(operand, source)
            {
                flow.assign(name, Position::of(operand));
            }
        }
        _ => {}
    }
}

fn target_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        VARIABLE_NAME | WORD => {
            let name = text(node, source);
            name.chars().all(is_variable_character).then(|| name.to_owned())
        }
        SUBSCRIPT => node
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source).to_owned()),
        _ => None,
    }
}

fn is_plain_assignment_target(node: Node<'_>, source: &[u8]) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == BINARY
            && parent.child_by_field_name(LEFT_FIELD) == Some(node)
            && parent
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == PLAIN_ASSIGNMENT)
    })
}

fn is_assignment_name(subscript: Node<'_>) -> bool {
    subscript.parent().is_some_and(|assignment| {
        assignment.kind() == ASSIGNMENT && assignment.child_by_field_name(NAME_FIELD) == Some(subscript)
    })
}

fn is_arithmetic_root(node: Node<'_>, source: &[u8]) -> bool {
    [ARITHMETIC_EXPANSION, ARITHMETIC_FOR].contains(&node.kind()) || is_arithmetic_command(node, source)
}

pub fn is_arithmetic_operand(node: Node<'_>, source: &[u8]) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == ARITHMETIC_FOR {
            return parent.child_by_field_name(BODY_FIELD) != Some(current);
        }
        if is_arithmetic_root(parent, source) {
            return true;
        }
        if parent.kind() == SUBSCRIPT {
            return parent.child_by_field_name(INDEX_FIELD) == Some(current)
                && (is_assignment_name(parent) || is_arithmetic_operand(parent, source));
        }
        if !EXPRESSION_KINDS.contains(&parent.kind()) {
            return false;
        }
        current = parent;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetTerm {
    pub name: String,
    pub offset: usize,
    pub assignment: Option<(usize, &'static str)>,
    pub stepped: bool,
}

pub fn let_terms(raw: &str) -> Vec<LetTerm> {
    let characters: Vec<(usize, char)> = raw.char_indices().collect();
    let mut terms = Vec::new();
    let mut index = 0;
    while let Some(&(offset, character)) = characters.get(index) {
        let after_name_character = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous))
            .is_some_and(|(_, previous)| is_variable_character(*previous) || *previous == DOLLAR);
        if !(character.is_ascii_alphabetic() || character == UNDERSCORE) || after_name_character {
            index = index.saturating_add(1);
            continue;
        }
        let end = characters
            .iter()
            .skip(index)
            .find(|(_, next)| !is_variable_character(*next))
            .map_or(raw.len(), |(end, _)| *end);
        let before = raw.get(..offset).unwrap_or_default().trim_end();
        let rest = raw.get(end..).unwrap_or_default();
        let trimmed = rest.trim_start();
        let operator_offset = end.saturating_add(rest.len().saturating_sub(trimmed.len()));
        let assignment = ASSIGNMENT_OPERATORS
            .iter()
            .rev()
            .find(|operator| trimmed.starts_with(**operator) && !trimmed.starts_with(EQUALITY))
            .map(|operator| (operator_offset, *operator));
        let stepped = STEPS
            .iter()
            .any(|step| trimmed.starts_with(step) || before.ends_with(step));
        terms.push(LetTerm {
            name: raw.get(offset..end).unwrap_or_default().to_owned(),
            offset,
            assignment,
            stepped,
        });
        index = characters
            .iter()
            .position(|(position, _)| *position >= end)
            .unwrap_or(characters.len());
    }
    terms
}

pub fn let_assignments(raw: &str) -> Vec<String> {
    let_terms(raw)
        .into_iter()
        .filter(|term| term.assignment.is_some() || term.stepped)
        .map(|term| term.name)
        .collect()
}

pub fn let_events(argument: Node<'_>, source: &[u8], flow: &mut Flow) {
    let base = Position::of(argument);
    for term in let_terms(text(argument, source)) {
        match term.assignment {
            Some((operator_offset, operator)) => {
                flow.assign(term.name.clone(), base.shifted(operator_offset));
                if operator != PLAIN_ASSIGNMENT {
                    flow.refer(term.name, base.shifted(term.offset));
                }
            }
            None => flow.refer(term.name, base.shifted(term.offset)),
        }
    }
}
