use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::effects::contains_effect;
use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, COMMENT, FALSE, FUNCTION_FIELD, LEFT_FIELD, PARENTHESIZED, RIGHT_FIELD, TRUE, is_bytes, is_string,
    named, string_prefix, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const BOOLEAN_OPERATOR: &str = "boolean_operator";
const NOT_OPERATOR: &str = "not_operator";
const UNARY_OPERATOR: &str = "unary_operator";
const CONDITIONAL: &str = "conditional_expression";
const ASSERT: &str = "assert_statement";
const IF_CLAUSE: &str = "if_clause";
const TESTED: [&str; 3] = ["if_statement", "elif_clause", "while_statement"];
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const CONDITION_FIELD: &str = "condition";
const OR: &str = "or";
const AND: &str = "and";
const NONE_LITERAL: &str = "none";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const ELLIPSIS: &str = "ellipsis";
const SIGNS: [&str; 2] = ["+", "-"];
const CONTAINERS: [&str; 4] = ["list", "tuple", "set", "dictionary"];
const SPLATS: [&str; 2] = ["list_splat", "dictionary_splat"];
const INITIALIZERS: [&str; 5] = ["set", "list", "tuple", "dict", "frozenset"];
const FORMATTED_PREFIXES: [char; 4] = ['f', 'F', 't', 'T'];
const NUMBER_NOISE: [char; 3] = ['_', 'j', 'J'];
const RADIX_PREFIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];

#[derive(Clone, Copy)]
struct Context<'model, 'tree, 'source> {
    model: &'model Model<'tree>,
    source: &'source [u8],
}

// SIM222 (`x or True or y`) y SIM223 (`x and False and y`): el `is_short_circuit` de ruff. Fuera de
// una condición sólo cuenta una constante que no es el último operando; avisa desde ahí.
pub fn short_circuit_reports(model: &Model<'_>, source: &[u8], operation: Node<'_>) -> Option<Report> {
    let operator = operator(operation, source)?;
    if operation
        .parent()
        .and_then(|parent| self::operator(parent, source))
        == Some(operator)
    {
        return None;
    }
    let (rule, short_circuits) = if operator == OR {
        (PythonRule::SIM222, true)
    } else {
        (PythonRule::SIM223, false)
    };
    let context = Context { model, source };
    let mut operands = Vec::new();
    flatten(operation, operator, source, &mut operands);
    let boolean_test = is_boolean_test(operation);
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    let mut location = operation.start_position();
    for pair in operands.windows(2) {
        let [value, next] = pair else {
            continue;
        };
        let value_truth = truthiness(*value, context);
        if value_truth.is_none() && (!boolean_test || contains_effect(*value, source, &is_builtin)) {
            location = next.start_position();
            continue;
        }
        if value_truth == Some(short_circuits) || truthiness(*next, context) == Some(short_circuits) {
            return Some(Report::at_point(rule, location, ""));
        }
        return None;
    }
    None
}

fn operator<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    if node.kind() != BOOLEAN_OPERATOR {
        return None;
    }
    let operator = text(node.child_by_field_name(OPERATOR_FIELD)?, source);
    [OR, AND].contains(&operator).then_some(operator)
}

fn flatten<'tree>(node: Node<'tree>, wanted: &str, source: &[u8], operands: &mut Vec<Node<'tree>>) {
    if operator(node, source) == Some(wanted) {
        for side in [LEFT_FIELD, RIGHT_FIELD] {
            if let Some(child) = node.child_by_field_name(side) {
                flatten(child, wanted, source, operands);
            }
        }
    } else {
        operands.push(node);
    }
}

// `in_boolean_test`: la condición de un `if`, `elif`, `while`, `assert`, un ternario, el `if` de una
// comprensión o el operando de un `not`.
fn is_boolean_test(operation: Node<'_>) -> bool {
    let mut child = operation;
    while let Some(parent) = child.parent() {
        let is_condition = |field: &str| {
            parent
                .child_by_field_name(field)
                .is_some_and(|condition| condition.id() == child.id())
        };
        match parent.kind() {
            PARENTHESIZED | BOOLEAN_OPERATOR => child = parent,
            NOT_OPERATOR | IF_CLAUSE => return true,
            kind if TESTED.contains(&kind) => return is_condition(CONDITION_FIELD),
            ASSERT => {
                return significant(parent)
                    .first()
                    .is_some_and(|test| test.id() == child.id());
            }
            CONDITIONAL => {
                return significant(parent)
                    .get(1)
                    .is_some_and(|test| test.id() == child.id());
            }
            _ => return false,
        }
    }
    false
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// `Truthiness::from_expr` de ruff: `Some(true)` si siempre es verdadero, `Some(false)` si siempre es
// falso, `None` si no se sabe.
fn truthiness(node: Node<'_>, context: Context<'_, '_, '_>) -> Option<bool> {
    let node = unparenthesized(node);
    let source = context.source;
    match node.kind() {
        TRUE => Some(true),
        FALSE | NONE_LITERAL => Some(false),
        INTEGER | FLOAT => Some(!is_zero(text(node, source))),
        kind if CONTAINERS.contains(&kind) => {
            let elements = significant(node);
            if elements.iter().any(|element| SPLATS.contains(&element.kind())) {
                None
            } else {
                Some(!elements.is_empty())
            }
        }
        UNARY_OPERATOR => {
            let sign = text(node.child_by_field_name(OPERATOR_FIELD)?, source);
            let operand = unparenthesized(node.child_by_field_name(ARGUMENT_FIELD)?);
            (SIGNS.contains(&sign) && matches!(operand.kind(), INTEGER | FLOAT))
                .then(|| !is_zero(text(operand, source)))
        }
        CALL => initializer_truthiness(node, context),
        _ if is_string(node) => string_truthiness(node, source),
        _ => None,
    }
}

// La veracidad de un literal suelto, sin contenedores ni llamadas: el `is_literal_expr` de ruff seguido
// de `Truthiness::from_expr`.
pub fn literal_truthiness(node: Node<'_>, source: &[u8]) -> Option<bool> {
    let node = unparenthesized(node);
    match node.kind() {
        TRUE | ELLIPSIS => Some(true),
        FALSE | NONE_LITERAL => Some(false),
        INTEGER | FLOAT => Some(!is_zero(text(node, source))),
        _ if is_string(node) => string_truthiness(node, source),
        _ => None,
    }
}

fn string_truthiness(node: Node<'_>, source: &[u8]) -> Option<bool> {
    if string_prefix(node, source).contains(FORMATTED_PREFIXES) {
        return None;
    }
    if is_bytes(node, source) {
        let literal = text(node, source);
        let body = literal
            .get(string_prefix(node, source).len()..)
            .unwrap_or(literal)
            .trim_matches(['"', '\'']);
        return Some(!body.is_empty());
    }
    text_value(node, source).map(|value| !value.is_empty())
}

fn initializer_truthiness(call: Node<'_>, context: Context<'_, '_, '_>) -> Option<bool> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let is_initializer = INITIALIZERS
        .iter()
        .any(|name| is_builtin_named(context.model, function, context.source, name));
    if !is_initializer {
        return None;
    }
    let given = arguments(call);
    match (given.positional.as_slice(), given.keywords.is_empty()) {
        ([], true) => Some(false),
        ([argument], true) if CONTAINERS.contains(&unparenthesized(*argument).kind()) => {
            truthiness(*argument, context)
        }
        _ => None,
    }
}

fn is_zero(literal: &str) -> bool {
    let digits = literal
        .chars()
        .filter(|digit| !NUMBER_NOISE.contains(digit))
        .collect::<String>()
        .to_ascii_lowercase();
    for (prefix, radix) in RADIX_PREFIXES {
        if let Some(rest) = digits.strip_prefix(prefix) {
            return u128::from_str_radix(rest, radix).is_ok_and(|value| value == 0);
        }
    }
    digits.parse::<f64>().is_ok_and(|value| value == 0.0)
}
