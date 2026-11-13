use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_null_flow::{
    AND, ARGUMENTS_FIELD, Analysis, BINARY, COMMENTS, EQUAL, INSTANCEOF, INVOCATION, LEFT_FIELD, NOT,
    NOT_EQUAL, NULL, OBJECT_FIELD, OPERAND_FIELD, OPERATOR_FIELD, OR, PARENTHESIZED, RIGHT_FIELD, State,
    UNARY,
};
use super::java_null_keys::{forget, join, tracked_key};
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
const FAILS_WHEN_FALSE: &[&str] = &[
    "checkState",
    "checkArgument",
    "checkRemove",
    "verify",
    "assertTrue",
];
const EAGER_AND: &str = "&";
const EAGER_OR: &str = "|";
const ASSIGNMENT: &str = "assignment_expression";

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
                .and_then(|left| tracked_key(analysis, left))
            {
                forget(&mut when_true, &tested);
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
                first_argument(node).and_then(|argument| tracked_key(analysis, argument)),
            ) {
                guard_by_call(&name, &argument, &mut when_true, &mut when_false);
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
        AND | EAGER_AND => {
            let (left_true, left_false) = branch_states(analysis, left, state);
            let (right_true, right_false) = branch_states(analysis, right, left_true);
            (
                right_true,
                join(Some(left_false), Some(right_false)).unwrap_or_default(),
            )
        }
        OR | EAGER_OR => {
            let (left_true, left_false) = branch_states(analysis, left, state);
            let (right_true, right_false) = branch_states(analysis, right, left_false);
            (
                join(Some(left_true), Some(right_true)).unwrap_or_default(),
                right_false,
            )
        }
        EQUAL | NOT_EQUAL => {
            let tested = match (left.kind(), right.kind()) {
                (_, NULL) => compared_key(analysis, left),
                (NULL, _) => compared_key(analysis, right),
                _ => None,
            };
            let after = analysis.expression(node, state);
            let Some(tested) = tested else {
                return (after.clone(), after);
            };
            let mut present = after.clone();
            forget(&mut present, &tested);
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
// Lo que se compara contra null: en `(x = campo) != null` el chequeo vale para `x`.
fn compared_key(analysis: &Analysis<'_, '_>, compared: Node<'_>) -> Option<String> {
    let inner = without_cast(compared);
    let target = if inner.kind() == ASSIGNMENT {
        inner.child_by_field_name(LEFT_FIELD)?
    } else {
        inner
    };
    tracked_key(analysis, target)
}

fn guard_by_call(name: &str, tested: &str, when_true: &mut State, when_false: &mut State) {
    if TRUE_MEANS_PRESENT.contains(&name) {
        forget(when_true, tested);
    } else if FALSE_MEANS_PRESENT.contains(&name) {
        forget(when_false, tested);
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

// `checkState(x != null, ...)` sigue sólo si la condición da verdadero.
pub(super) fn asserted_condition(
    analysis: &mut Analysis<'_, '_>,
    call: Node<'_>,
    condition: Node<'_>,
    state: State,
) -> State {
    let mut current = branch_states(analysis, condition, state).0;
    for argument in call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.id() != condition.id())
    {
        current = analysis.expression(argument, current);
    }
    current
}

// La condición de una sentencia que falla si da falso: `checkState(x != null, "...")`.
pub(super) fn condition_asserted<'tree>(expression: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let name = expression
        .child_by_field_name(NAME_FIELD)
        .filter(|_| expression.kind() == INVOCATION)?;
    FAILS_WHEN_FALSE
        .contains(&text(name, source))
        .then(|| first_argument(expression))
        .flatten()
}

// Una sentencia que asegura que la variable no es null: `requireNonNull(x)`, `assertNotNull(x)`,
// `assertThat(x).isNotNull()`.
pub(super) fn asserted_present<'tree>(expression: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if expression.kind() != INVOCATION {
        return None;
    }
    let name = text(expression.child_by_field_name(NAME_FIELD)?, source);
    if ASSERTS_PRESENT.contains(&name) {
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
    }
}
