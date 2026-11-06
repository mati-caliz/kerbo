use tree_sitter::Node;

use super::nodes::{IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, operator, text, without_parentheses};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const NUMBER: &str = "number";
const ARGUMENT_FIELD: &str = "argument";
const FOR: &str = "for_statement";
const CONDITION_FIELD: &str = "condition";
const INCREMENT_FIELD: &str = "increment";
const UPDATE: &str = "update_expression";
const AUGMENTED: &str = "augmented_assignment_expression";
const ASSIGNMENT: &str = "assignment_expression";
const SEQUENCE: &str = "sequence_expression";
const INCREMENT: &str = "++";
const DECREMENT: &str = "--";
const ADD_ASSIGN: &str = "+=";
const SUBTRACT_ASSIGN: &str = "-=";
const LESS: &[&str] = &["<", "<="];
const GREATER: &[&str] = &[">", ">="];
const MINUS: &str = "-";
const PLUS: &str = "+";
const TRUE: &str = "true";
const BIGINT_SUFFIX: char = 'n';

// `for-direction`: un contador que se aleja de la condición de corte.
pub fn wrong_for_directions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[FOR])
        .flat_map(|loop_node| {
            let count = wrong_counters(loop_node, context.source);
            std::iter::repeat_n(report(EslintRule::ForDirection, loop_node, ""), count)
        })
        .collect()
}

// Cuántos lados de la condición son un contador que va para el lado equivocado.
fn wrong_counters(loop_node: Node<'_>, source: &[u8]) -> usize {
    let (Some(test), Some(update)) = (
        loop_node
            .child_by_field_name(CONDITION_FIELD)
            .map(without_parentheses)
            .filter(|test| test.kind() == BINARY),
        loop_node.child_by_field_name(INCREMENT_FIELD),
    ) else {
        return 0;
    };
    let test_operator = operator(test, source);
    let Some(wrong_when_left) = wrong_direction_on_the_left(test_operator) else {
        return 0;
    };
    [(LEFT_FIELD, wrong_when_left), (RIGHT_FIELD, -wrong_when_left)]
        .into_iter()
        .filter(|(field, wrong)| {
            test.child_by_field_name(field)
                .filter(|counter| counter.kind() == IDENTIFIER)
                .is_some_and(|counter| {
                    let name = text(counter, source);
                    matches!(modifying_expressions(update, name, source)[..],
                        [only] if direction_of(only, name, source) == *wrong)
                })
        })
        .count()
}

fn wrong_direction_on_the_left(test_operator: &str) -> Option<i32> {
    if LESS.contains(&test_operator) {
        Some(-1)
    } else if GREATER.contains(&test_operator) {
        Some(1)
    } else {
        None
    }
}

fn modifying_expressions<'tree>(node: Node<'tree>, counter: &str, source: &[u8]) -> Vec<Node<'tree>> {
    let node = without_parentheses(node);
    match node.kind() {
        SEQUENCE => (0..node.named_child_count())
            .filter_map(|index| {
                u32::try_from(index)
                    .ok()
                    .and_then(|index| node.named_child(index))
            })
            .flat_map(|part| modifying_expressions(part, counter, source))
            .collect(),
        UPDATE => node
            .child_by_field_name(ARGUMENT_FIELD)
            .filter(|argument| argument.kind() == IDENTIFIER && text(*argument, source) == counter)
            .map_or_else(Vec::new, |_| vec![node]),
        AUGMENTED | ASSIGNMENT => node
            .child_by_field_name(LEFT_FIELD)
            .filter(|left| left.kind() == IDENTIFIER && text(*left, source) == counter)
            .map_or_else(Vec::new, |_| vec![node]),
        _ => Vec::new(),
    }
}

fn direction_of(change: Node<'_>, counter: &str, source: &[u8]) -> i32 {
    let change_operator = operator(change, source);
    if change.kind() == UPDATE {
        return match change_operator {
            INCREMENT => 1,
            DECREMENT => -1,
            _ => 0,
        };
    }
    let is_counter = change
        .child_by_field_name(LEFT_FIELD)
        .is_some_and(|left| text(left, source) == counter);
    let direction = match change_operator {
        ADD_ASSIGN if is_counter => 1,
        SUBTRACT_ASSIGN if is_counter => -1,
        _ => return 0,
    };
    change
        .child_by_field_name(RIGHT_FIELD)
        .map_or(0, |right| direction * static_sign(right, source))
}

// El signo de `getStaticValue`, en los valores que se pueden leer sin evaluar el programa.
fn static_sign(node: Node<'_>, source: &[u8]) -> i32 {
    let node = without_parentheses(node);
    match node.kind() {
        NUMBER => {
            let raw = text(node, source)
                .trim_end_matches(BIGINT_SUFFIX)
                .replace('_', "");
            let value = raw.parse::<f64>().unwrap_or(f64::NAN);
            i32::from(!value.is_nan() && value != 0.0)
        }
        TRUE => 1,
        UNARY => {
            let inner = node
                .child_by_field_name(ARGUMENT_FIELD)
                .map_or(0, |argument| static_sign(argument, source));
            match operator(node, source) {
                MINUS => -inner,
                PLUS => inner,
                _ => 0,
            }
        }
        _ => 0,
    }
}
