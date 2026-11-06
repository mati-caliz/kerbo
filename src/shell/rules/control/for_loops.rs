use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::Findings;
use crate::shell::syntax::{
    ARITHMETIC_FOR, ASSIGNMENT, BINARY, FOR, LEFT_FIELD, NAME_FIELD, OPERATOR_FIELD, VALUE_FIELD,
    VARIABLE_FIELD, ancestors, children, field_children, text,
};
use crate::shell::word_expansion::{may_become_multiple, will_split};
use crate::shell::word_pieces::{Piece, PieceKind, pieces, unquoted_literal};

const INITIALIZER_FIELD: &str = "initializer";
const SELECT: &str = "select";
const DISCARDED_VARIABLE: &str = "_";
const ELEMENT_COMMA: char = ',';
const GLOB_STAR: char = '*';
const ASSIGN: &str = "=";

pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() == FOR && !is_select(node, source) {
        check_values(node, source, findings);
    }
    if [FOR, ARITHMETIC_FOR].contains(&node.kind()) {
        check_reassigned_variable(node, source, findings);
    }
}

fn is_select(node: Node<'_>, source: &[u8]) -> bool {
    children(node)
        .first()
        .is_some_and(|keyword| text(*keyword, source) == SELECT)
}

// SC2041, SC2042, SC2043, SC2066 y SC2258: `checkForInQuoted`.
fn check_values(loop_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let values = field_children(loop_node, VALUE_FIELD);
    if let [only] = values[..] {
        let word_pieces = pieces(only, source);
        if check_single_value(only, &word_pieces, findings) {
            return;
        }
    }
    for value in values {
        let word_pieces = pieces(value, source);
        let trailing_comma = word_pieces
            .last()
            .is_some_and(|last| last.kind == PieceKind::Literal && last.text.ends_with(ELEMENT_COMMA));
        if trailing_comma {
            findings.report(ShellRule::Sc2258, value, text(value, source));
        }
    }
}

// Devuelve si la cláusula de ShellCheck que corresponde ya decidió, y no hay que seguir con las demás.
fn check_single_value(value: Node<'_>, word_pieces: &[Piece], findings: &mut Findings) -> bool {
    if let [only] = word_pieces {
        if only.kind == PieceKind::DoubleQuoted
            && ((only.inner.iter().any(will_split) && !may_become_multiple(word_pieces))
                || only
                    .literal_string()
                    .is_some_and(|literal| literal.contains(GLOB_STAR)))
        {
            findings.report_at(ShellRule::Sc2066, only.position, "");
            return true;
        }
        if only.kind == PieceKind::SingleQuoted {
            findings.report_at(ShellRule::Sc2041, only.position, "");
            return true;
        }
    }
    if unquoted_literal(word_pieces).is_some_and(|literal| literal.contains(ELEMENT_COMMA)) {
        findings.report(ShellRule::Sc2042, value, "");
        return true;
    }
    if !word_pieces.iter().any(will_split) && !may_become_multiple(word_pieces) {
        findings.report(ShellRule::Sc2043, value, "");
        return true;
    }
    false
}

// SC2165 y SC2167: `checkLoopVariableReassignment`.
fn check_reassigned_variable(loop_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(variable) = loop_variable(loop_node, source) else {
        return;
    };
    if variable == DISCARDED_VARIABLE {
        return;
    }
    let parent = ancestors(loop_node).find(|ancestor| loop_variable(*ancestor, source) == Some(variable));
    if let Some(parent) = parent {
        findings.report(ShellRule::Sc2165, loop_node, variable);
        findings.report(ShellRule::Sc2167, parent, variable);
    }
}

fn loop_variable<'source>(loop_node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    match loop_node.kind() {
        FOR if !is_select(loop_node, source) => loop_node
            .child_by_field_name(VARIABLE_FIELD)
            .map(|variable| text(variable, source)),
        ARITHMETIC_FOR => {
            let [initializer] = field_children(loop_node, INITIALIZER_FIELD)[..] else {
                return None;
            };
            if initializer.kind() == ASSIGNMENT {
                return initializer
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source));
            }
            let is_assignment = initializer.kind() == BINARY
                && initializer
                    .child_by_field_name(OPERATOR_FIELD)
                    .is_some_and(|operator| text(operator, source) == ASSIGN);
            is_assignment
                .then(|| initializer.child_by_field_name(LEFT_FIELD))
                .flatten()
                .map(|left| text(left, source))
        }
        _ => None,
    }
}
