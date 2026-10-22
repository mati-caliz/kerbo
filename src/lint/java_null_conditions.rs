use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_null_flow::{
    AND, ARGUMENTS_FIELD, Analysis, BINARY, COMMENTS, EQUAL, IDENTIFIER, INSTANCEOF, INVOCATION, LEFT_FIELD,
    NOT, NOT_EQUAL, NULL, OBJECT_FIELD, OPERAND_FIELD, OPERATOR_FIELD, OR, PARENTHESIZED, RIGHT_FIELD, State,
    UNARY, join,
};
use crate::syntax::named_children;

// Llamadas que, si dan verdadero, prueban que su primer argumento no es null, y las que lo prueban
// si dan falso.
// Las llamadas que como sentencia aseguran que su argumento no es null.
const CAST: &str = "cast_expression";
const ASSERTS_PRESENT: &[&str] = &["requireNonNull", "assertNotNull", "notNull", "checkNotNull"];
const ASSERT_THAT: &str = "assertThat";
const IS_NOT_NULL: &str = "isNotNull";
const VALUE_FIELD: &str = "value";
const TRUE_MEANS_PRESENT: &[&str] = &["nonNull", "hasText", "hasLength", "isNotEmpty", "isNotBlank"];
const FALSE_MEANS_PRESENT: &[&str] = &["isNull", "isEmpty", "isBlank"];

// Los estados si la condición da verdadero y si da falso.
pub(super) fn branch_states(analysis: &mut Analysis<'_, '_>, node: Node<'_>, state: State) -> (State, State) {
    match node.kind() {
        PARENTHESIZED => match named_children(node)
            .into_iter()
            .find(|child| !COMMENTS.contains(&child.kind()))
        {
            Some(inner) => branch_states(analysis, inner, state),
            None => (state.clone(), state),
        },
        UNARY
            if node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| operator.kind() == NOT) =>
        {
            match node.child_by_field_name(OPERAND_FIELD) {
                Some(operand) => {
                    let (when_true, when_false) = branch_states(analysis, operand, state);
                    (when_false, when_true)
                }
                None => (state.clone(), state),
            }
        }
        BINARY => binary_condition(analysis, node, state),
        INSTANCEOF => {
            let after = analysis.expression(node, state);
            let mut when_true = after.clone();
            if let Some(tested) = node
                .child_by_field_name(LEFT_FIELD)
                .filter(|left| left.kind() == IDENTIFIER)
            {
                when_true.remove(analysis.text(tested));
            }
            (when_true, after)
        }
        INVOCATION => {
            let after = analysis.expression(node, state);
            let (mut when_true, mut when_false) = (after.clone(), after);
            let name = node
                .child_by_field_name(NAME_FIELD)
                .map(|name| analysis.text(name).to_owned());
            if let (Some(name), Some(argument)) = (
                name,
                first_argument(node).filter(|argument| argument.kind() == IDENTIFIER),
            ) {
                guard_by_call(&name, analysis.text(argument), &mut when_true, &mut when_false);
            }
            (when_true, when_false)
        }
        _ => {
            let after = analysis.expression(node, state);
            (after.clone(), after)
        }
    }
}

fn binary_condition(analysis: &mut Analysis<'_, '_>, node: Node<'_>, state: State) -> (State, State) {
    let operator = node
        .child_by_field_name(OPERATOR_FIELD)
        .map(|operator| operator.kind())
        .unwrap_or_default();
    let (Some(left), Some(right)) = (
        node.child_by_field_name(LEFT_FIELD),
        node.child_by_field_name(RIGHT_FIELD),
    ) else {
        return (state.clone(), state);
    };
    match operator {
        AND => {
            let (left_true, left_false) = branch_states(analysis, left, state);
            let (right_true, right_false) = branch_states(analysis, right, left_true);
            (
                right_true,
                join(Some(left_false), Some(right_false)).unwrap_or_default(),
            )
        }
        OR => {
            let (left_true, left_false) = branch_states(analysis, left, state);
            let (right_true, right_false) = branch_states(analysis, right, left_false);
            (
                join(Some(left_true), Some(right_true)).unwrap_or_default(),
                right_false,
            )
        }
        EQUAL | NOT_EQUAL => {
            let tested = match (left.kind(), right.kind()) {
                (IDENTIFIER, NULL) => Some(left),
                (NULL, IDENTIFIER) => Some(right),
                _ => None,
            };
            let after = analysis.expression(node, state);
            let Some(tested) = tested else {
                return (after.clone(), after);
            };
            let mut present = after.clone();
            present.remove(analysis.text(tested));
            if operator == NOT_EQUAL {
                (present, after)
            } else {
                (after, present)
            }
        }
        _ => {
            let after = analysis.expression(node, state);
            (after.clone(), after)
        }
    }
}
fn guard_by_call(name: &str, tested: &str, when_true: &mut State, when_false: &mut State) {
    if TRUE_MEANS_PRESENT.contains(&name) {
        when_true.remove(tested);
    } else if FALSE_MEANS_PRESENT.contains(&name) {
        when_false.remove(tested);
    }
}

pub(super) fn is_logical(node: Node<'_>) -> bool {
    node.child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| operator.kind() == AND || operator.kind() == OR)
}

pub(super) fn first_argument(invocation: Node<'_>) -> Option<Node<'_>> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .find(|argument| !COMMENTS.contains(&argument.kind()))
}

// `(Tipo) llamada()` vale lo que la llamada.
pub(super) fn without_cast(value: Node<'_>) -> Node<'_> {
    let mut current = value;
    while current.kind() == CAST || current.kind() == PARENTHESIZED {
        let Some(inner) = current.child_by_field_name(VALUE_FIELD).or_else(|| {
            named_children(current)
                .into_iter()
                .find(|child| !COMMENTS.contains(&child.kind()))
        }) else {
            break;
        };
        current = inner;
    }
    current
}

// Una sentencia que asegura que la variable no es null: `requireNonNull(x)`, `assertNotNull(x)`,
// `assertThat(x).isNotNull()`.
pub(super) fn asserted_present<'tree>(expression: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if expression.kind() != INVOCATION {
        return None;
    }
    let name = text(expression.child_by_field_name(NAME_FIELD)?, source);
    let argument = if ASSERTS_PRESENT.contains(&name) {
        first_argument(expression)
    } else if name == IS_NOT_NULL {
        expression
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| {
                object.kind() == INVOCATION
                    && object
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|inner| text(inner, source) == ASSERT_THAT)
            })
            .and_then(first_argument)
    } else {
        None
    };
    argument.filter(|argument| argument.kind() == IDENTIFIER)
}
