use tree_sitter::Node;

use super::constants::{Constants, TRUE};
use super::nodes::{CONDITION_FIELD, IF, field_children, without_parentheses};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const TERNARY: &str = "ternary_expression";
const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const FOR: &str = "for_statement";
const EMPTY_STATEMENT: &str = "empty_statement";
const INITIALIZER_FIELD: &str = "initializer";
const YIELD: &str = "yield_expression";
// Los que abren un conjunto nuevo de loops en ESLint: las funciones que no son flecha.
const GENERATOR_BOUNDARIES: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
];

// `no-constant-condition` con `checkLoops: "allExceptWhileTrue"`: la condición de un `if`, de un
// ternario o de un loop que siempre da lo mismo. Un loop con un `yield` adentro se perdona.
pub fn constant_conditions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    let branches = context
        .of_kind(&[IF, TERNARY])
        .filter_map(|node| node.child_by_field_name(CONDITION_FIELD));
    let loops = context
        .of_kind(&[WHILE, DO, FOR])
        .filter(|node| !is_while_true(*node))
        .filter(|node| !yields_inside(*node))
        .filter_map(loop_test);
    branches
        .chain(loops)
        .map(without_parentheses)
        .filter(|test| constants.is_constant(*test, true))
        .map(|test| report(EslintRule::NoConstantCondition, test, ""))
        .collect()
}

fn loop_test(node: Node<'_>) -> Option<Node<'_>> {
    field_children(node, CONDITION_FIELD)
        .into_iter()
        .find(|condition| condition.is_named() && condition.kind() != EMPTY_STATEMENT)
}

fn is_while_true(node: Node<'_>) -> bool {
    node.kind() == WHILE
        && node
            .child_by_field_name(CONDITION_FIELD)
            .map(without_parentheses)
            .is_some_and(|test| test.kind() == TRUE)
}

// Un `yield` del mismo generador entre la condición y el final del loop vacía el conjunto de loops
// pendientes de ESLint; en un `for`, el de la inicialización no cuenta porque la condición vuelve a
// anotar el loop.
fn yields_inside(node: Node<'_>) -> bool {
    let initializer = node
        .child_by_field_name(INITIALIZER_FIELD)
        .filter(|_| node.kind() == FOR);
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == YIELD && initializer.is_none_or(|skipped| !contains(skipped, current)) {
            return true;
        }
        let mut cursor = current.walk();
        pending.extend(
            current
                .named_children(&mut cursor)
                .filter(|child| !GENERATOR_BOUNDARIES.contains(&child.kind())),
        );
    }
    false
}

fn contains(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}
