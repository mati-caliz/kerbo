use tree_sitter::Node;

use super::statements::Interpreter;
use crate::lint::taint::folding::{fold_binary, truth};
use crate::lint::taint::{Literal, State, Value};
use crate::syntax::named_children;

const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OPERATORS_FIELD: &str = "operators";
const PLUS: &str = "+";
const PERCENT: &str = "%";
const SLASH: &str = "/";
const IN: &str = "in";
const NOT_IN: &str = "not in";
const AND: &str = "and";
const OR: &str = "or";
const BOTH: &str = "&&";
const EITHER: &str = "||";

pub(super) fn binary(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let left = node
        .child_by_field_name(LEFT_FIELD)
        .map_or_else(Value::clean, |inner| interpreter.expression(inner, state));
    let right = node
        .child_by_field_name(RIGHT_FIELD)
        .map_or_else(Value::clean, |inner| interpreter.expression(inner, state));
    let operator = node
        .child_by_field_name(OPERATOR_FIELD)
        .map(|written| interpreter.text(written))
        .unwrap_or_default();
    if let Some(folded) = left
        .literal()
        .zip(right.literal())
        .and_then(|(one, two)| fold_binary(operator, one, two))
    {
        return Value::Constant(folded);
    }
    if operator == PLUS || operator == PERCENT || operator == SLASH {
        Value::Data(left.flow().union(&right.flow()))
    } else {
        Value::clean()
    }
}

// `a or b` devuelve uno de los dos, no un booleano: lleva lo de ambos.
pub(super) fn boolean(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let left = node
        .child_by_field_name(LEFT_FIELD)
        .map_or_else(Value::clean, |inner| interpreter.expression(inner, state));
    let right = node
        .child_by_field_name(RIGHT_FIELD)
        .map_or_else(Value::clean, |inner| interpreter.expression(inner, state));
    let operator = match node
        .child_by_field_name(OPERATOR_FIELD)
        .map(|written| interpreter.text(written))
    {
        Some(AND) => BOTH,
        Some(OR) => EITHER,
        _ => return Value::Data(left.flow().union(&right.flow())),
    };
    match (left.literal(), right.literal()) {
        (Some(Literal::Boolean(one)), Some(Literal::Boolean(two))) => {
            fold_binary(operator, &Literal::Boolean(*one), &Literal::Boolean(*two))
                .map_or_else(Value::clean, Value::Constant)
        }
        _ => Value::Data(left.flow().union(&right.flow())),
    }
}

pub(super) fn comparison(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let operands: Vec<Value> = named_children(node)
        .into_iter()
        .map(|operand| interpreter.expression(operand, state))
        .collect();
    let mut cursor = node.walk();
    let operators: Vec<&str> = node
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| interpreter.text(operator))
        .collect();
    let ([one, two], [operator]) = (operands.as_slice(), operators.as_slice()) else {
        return Value::clean();
    };
    let (Some(left), Some(right)) = (one.literal(), two.literal()) else {
        return Value::clean();
    };
    let contained = |needle: &Literal, haystack: &Literal| match (needle, haystack) {
        (Literal::Text(small), Literal::Text(big)) => Some(big.contains(small.as_str())),
        _ => None,
    };
    let decided = match *operator {
        IN => contained(left, right),
        NOT_IN => contained(left, right).map(|found| !found),
        other => truth(fold_binary(other, left, right).as_ref()),
    };
    decided.map_or_else(Value::clean, |flag| Value::Constant(Literal::Boolean(flag)))
}

pub(super) fn conditional(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let parts = named_children(node);
    let [taken, condition, other] = parts.as_slice() else {
        return Value::clean();
    };
    let decided = truth(interpreter.expression(*condition, state).literal());
    match decided {
        Some(true) => interpreter.expression(*taken, state),
        Some(false) => interpreter.expression(*other, state),
        None => {
            let first = interpreter.expression(*taken, state);
            first.join(interpreter.expression(*other, state))
        }
    }
}
