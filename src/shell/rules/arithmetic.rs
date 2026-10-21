use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ASSIGNMENT, BINARY, EXPANSIONS, NAME_FIELD, PARENTHESIZED, POSTFIX,
    SUBSCRIPT, TERNARY, UNARY, is_arithmetic_command,
};
use crate::shell::words::braced_content;

const EXCEPTION_CHARACTERS: &[char] = &[
    '/', '.', ':', '#', '%', '?', '*', '@', '$', '-', '!', '+', '=', '^', ',',
];
const EXPRESSION_KINDS: &[&str] = &[BINARY, UNARY, POSTFIX, PARENTHESIZED, TERNARY];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if !EXPANSIONS.contains(&node.kind()) || !is_arithmetic_operand(node, context) {
        return;
    }
    let braced = braced_content(node, context.source);
    let exempt = braced.is_empty()
        || braced.contains(EXCEPTION_CHARACTERS)
        || braced.starts_with(|first: char| first.is_ascii_digit());
    if !exempt {
        findings.report(ShellRule::Sc2004, node, context.text(node));
    }
}

fn is_arithmetic_operand(node: Node<'_>, context: &Context<'_, '_>) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if [ARITHMETIC_EXPANSION, ARITHMETIC_FOR].contains(&parent.kind())
            || is_arithmetic_command(parent, context.source)
        {
            return true;
        }
        if parent.kind() == SUBSCRIPT {
            return is_assignment_index(parent) && !is_associative(parent, context);
        }
        if !EXPRESSION_KINDS.contains(&parent.kind()) {
            return false;
        }
        current = parent;
    }
    false
}

fn is_assignment_index(subscript: Node<'_>) -> bool {
    subscript.parent().is_some_and(|assignment| {
        assignment.kind() == ASSIGNMENT && assignment.child_by_field_name(NAME_FIELD) == Some(subscript)
    })
}

fn is_associative(subscript: Node<'_>, context: &Context<'_, '_>) -> bool {
    subscript
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| context.associative.contains(context.text(name)))
}
