use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, FUNCTION_FIELD, LEFT_FIELD, LIST, RIGHT_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const SUM: &str = "sum";
const INT: &str = "int";
const ROUND: &str = "round";
const START: &str = "start";
const NDIGITS: &str = "ndigits";
const INTEGER: &str = "integer";
const NONE_LITERAL: &str = "none";
const UNARY_OPERATOR: &str = "unary_operator";
const ARGUMENT_FIELD: &str = "argument";
const BINARY_OPERATOR: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const INTEGER_OPERATORS: [&str; 11] = ["+", "-", "*", "//", "%", "**", "<<", ">>", "&", "|", "^"];
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const INTEGER_RESULTS: [&str; 11] = [
    "builtins.len",
    "builtins.id",
    "builtins.hash",
    "builtins.ord",
    "builtins.int",
    "math.comb",
    "math.factorial",
    "math.gcd",
    "math.lcm",
    "math.isqrt",
    "math.perm",
];
const ROUNDING: [&str; 3] = ["math.ceil", "math.floor", "math.trunc"];

// RUF017 (`sum(listas, [])`, que copia la lista acumulada en cada paso) y RUF046 (`int(...)` de algo
// que ya es un entero).
pub fn integer_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let mut reports = Vec::new();
    if is_builtin_named(model, function, source, SUM)
        && !given.positional.is_empty()
        && argument(&given, START, 1, source).is_some_and(|start| is_empty_list(model, source, start))
    {
        reports.push(Report::at(PythonRule::RUF017, call, ""));
    }
    if is_builtin_named(model, function, source, INT)
        && given.keywords.is_empty()
        && let [value] = given.positional.as_slice()
        && is_integer(model, source, unparenthesized(*value))
    {
        reports.push(Report::at(PythonRule::RUF046, call, ""));
    }
    reports
}

// `find_argument_value` de ruff: por nombre o por posición.
fn argument<'tree>(
    given: &Arguments<'tree>,
    name: &str,
    position: usize,
    source: &[u8],
) -> Option<Node<'tree>> {
    keyword_value(given, name, source).or_else(|| given.positional.get(position).copied())
}

fn is_empty_list(model: &Model<'_>, source: &[u8], start: Node<'_>) -> bool {
    let start = unparenthesized(start);
    match start.kind() {
        LIST => named(start).is_empty(),
        CALL => {
            start
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| is_builtin_named(model, function, source, LIST))
                && {
                    let given = arguments(start);
                    given.positional.is_empty() && given.keywords.is_empty()
                }
        }
        _ => false,
    }
}

fn is_integer(model: &Model<'_>, source: &[u8], value: Node<'_>) -> bool {
    if value.kind() == CALL {
        is_integer_call(model, source, value)
    } else {
        is_integer_literal(value, source)
    }
}

// `ResolvedPythonType` de ruff para los enteros: un entero escrito, o una operación entre enteros
// que no sea `/`.
fn is_integer_literal(value: Node<'_>, source: &[u8]) -> bool {
    let operand_is_integer = |field: &str| {
        value
            .child_by_field_name(field)
            .is_some_and(|operand| is_integer_literal(unparenthesized(operand), source))
    };
    match value.kind() {
        INTEGER => !text(value, source).ends_with(IMAGINARY_SUFFIXES),
        UNARY_OPERATOR => operand_is_integer(ARGUMENT_FIELD),
        BINARY_OPERATOR => {
            value
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| INTEGER_OPERATORS.contains(&text(operator, source)))
                && operand_is_integer(LEFT_FIELD)
                && operand_is_integer(RIGHT_FIELD)
        }
        _ => false,
    }
}

// `call_applicability` de ruff: lo que siempre devuelve un `int`, `round` sin decimales y los
// redondeos de `math`.
fn is_integer_call(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    let Some(name) = call
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|function| qualified_name(model, function, source))
    else {
        return false;
    };
    if INTEGER_RESULTS.contains(&name.as_str()) || ROUNDING.contains(&name.as_str()) {
        return true;
    }
    if name.strip_prefix("builtins.") != Some(ROUND) {
        return false;
    }
    let given = arguments(call);
    let Some(number) = argument(&given, "number", 0, source) else {
        return false;
    };
    match argument(&given, NDIGITS, 1, source).map(unparenthesized) {
        None => true,
        Some(ndigits) if ndigits.kind() == NONE_LITERAL => true,
        Some(ndigits) => {
            is_integer_literal(unparenthesized(number), source) && is_integer_literal(ndigits, source)
        }
    }
}
