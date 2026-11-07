use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ASSIGNMENT, BINARY, EXPANSIONS, NAME_FIELD, NUMBER, PARENTHESIZED,
    POSTFIX, SUBSCRIPT, TERNARY, UNARY, is_arithmetic_command,
};
use crate::shell::words::braced_content;

const EXCEPTION_CHARACTERS: &[char] = &[
    '/', '.', ':', '#', '%', '?', '*', '@', '$', '-', '!', '+', '=', '^', ',',
];
const EXPRESSION_KINDS: &[&str] = &[BINARY, UNARY, POSTFIX, PARENTHESIZED, TERNARY];
const DECIMAL_POINT: char = '.';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    check_decimal(node, context, findings);
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

// SC2079: `checkForDecimals`. El índice de `a[1.2]=x` lo mira la regla de asignaciones.
fn check_decimal(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let literal = context.text(node);
    let is_decimal =
        literal.starts_with(|first: char| first.is_ascii_digit()) && literal.contains(DECIMAL_POINT);
    if node.kind() == NUMBER
        && node.named_child_count() == 0
        && is_decimal
        && context.dialect != Dialect::Ksh
        && node.parent().is_some_and(|parent| parent.kind() != SUBSCRIPT)
        && is_arithmetic_operand(node, context)
    {
        findings.report(ShellRule::Sc2079, node, literal);
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
