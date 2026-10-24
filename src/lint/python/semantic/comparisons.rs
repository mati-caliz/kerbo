use tree_sitter::Node;

use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use super::version_checks::version_comparison;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, COMMENT, CONCATENATED_STRING, FALSE, FUNCTION_FIELD, IDENTIFIER, NONE, STRING, TRUE, is_f_string,
    is_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const ARGUMENTS_FIELD: &str = "arguments";
const NUMBERS: [&str; 2] = ["integer", "float"];
const ELLIPSIS: &str = "ellipsis";
const FLOAT_BUILTIN: &str = "float";
const VERSION_INFO: &str = "sys.version_info";
const OPERATORS_FIELD: &str = "operators";
const VALUE_FIELD: &str = "value";
const SUBSCRIPT: &str = "subscript";
const INTEGER: &str = "integer";
const MAJOR: &str = "3";
const EQUALITY: [&str; 2] = ["==", "!="];
const ORDERINGS: [&str; 4] = ["<", "<=", ">", ">="];
const NAN_TEXTS: [&str; 3] = ["nan", "+nan", "-nan"];
const NAN_CONSTANTS: [&str; 4] = ["math.nan", "numpy.nan", "numpy.NaN", "numpy.NAN"];
const IDEMPOTENT_BUILTINS: [&str; 8] = ["id", "len", "type", "int", "bool", "str", "repr", "bytes"];

// PLR0124 (comparar algo consigo mismo) y PLR0133 (comparar dos constantes), por cada par de una
// comparación encadenada, y PLW0177 (comparar con NaN).
pub fn comparison_reports(model: &Model<'_>, source: &[u8], comparison: Node<'_>) -> Vec<Report> {
    let operands: Vec<Node<'_>> = named(comparison)
        .into_iter()
        .filter(|operand| operand.kind() != COMMENT)
        .map(unparenthesized)
        .collect();
    let mut reports: Vec<Report> = operands
        .iter()
        .filter(|operand| is_nan(model, source, **operand))
        .map(|operand| Report::at(PythonRule::PLW0177, *operand, ""))
        .collect();
    reports.extend(version_info_comparison(model, source, comparison, &operands));
    reports.extend(version_comparison(model, source, comparison));
    for pair in operands.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if is_same_name(*left, *right, source) || is_same_builtin_call(model, source, *left, *right) {
            reports.push(Report::at(PythonRule::PLR0124, *left, ""));
        }
        if is_literal(*left, source) && is_literal(*right, source) {
            reports.push(Report::at(PythonRule::PLR0133, *left, ""));
        }
    }
    reports
}

// PLW0177: `float("nan")`, `math.nan` o `numpy.nan`, que nunca son iguales a nada.
fn is_nan(model: &Model<'_>, source: &[u8], operand: Node<'_>) -> bool {
    if qualified_name(model, operand, source).is_some_and(|name| NAN_CONSTANTS.contains(&name.as_str())) {
        return true;
    }
    operand.kind() == CALL
        && operand
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(model, function, source, FLOAT_BUILTIN))
        && operand
            .child_by_field_name(ARGUMENTS_FIELD)
            .map(|list| named(list).into_iter().filter(|argument| argument.kind() != COMMENT).collect::<Vec<_>>())
            .is_some_and(|given| {
                matches!(given.as_slice(), [only] if is_string(*only)
                    && text_value(*only, source).is_some_and(|value| NAN_TEXTS.contains(&value.to_ascii_lowercase().as_str())))
            })
}

// YTT201 (`sys.version_info[0] == 3`) y YTT203 (`sys.version_info[1] < 7`), que se rompen con
// Python 4. Avisa en el subíndice.
fn version_info_comparison(
    model: &Model<'_>,
    source: &[u8],
    comparison: Node<'_>,
    operands: &[Node<'_>],
) -> Option<Report> {
    let [left, right] = operands else {
        return None;
    };
    let mut cursor = comparison.walk();
    let operators: Vec<&str> = comparison
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    let [operator] = operators.as_slice() else {
        return None;
    };
    let indexes = left.child_by_field_name(VALUE_FIELD).filter(|value| {
        left.kind() == SUBSCRIPT && qualified_name(model, *value, source).as_deref() == Some(VERSION_INFO)
    })?;
    let index = named(*left)
        .into_iter()
        .filter(|part| part.kind() != COMMENT && part.id() != indexes.id())
        .map(|part| text(part, source))
        .collect::<Vec<_>>();
    let is_integer = right.kind() == INTEGER;
    let rule = match index.as_slice() {
        ["0"] if EQUALITY.contains(operator) && text(*right, source) == MAJOR => PythonRule::YTT201,
        ["1"] if ORDERINGS.contains(operator) && is_integer => PythonRule::YTT203,
        _ => return None,
    };
    Some(Report::at(rule, *left, ""))
}

fn is_same_name(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    left.kind() == IDENTIFIER && right.kind() == IDENTIFIER && text(left, source) == text(right, source)
}

// `len(x) == len(x)` y compañía, con el mismo builtin y el mismo nombre como único argumento.
fn is_same_builtin_call(model: &Model<'_>, source: &[u8], left: Node<'_>, right: Node<'_>) -> bool {
    let (Some((left_function, left_argument)), Some((right_function, right_argument))) =
        (single_name_call(left), single_name_call(right))
    else {
        return false;
    };
    text(left_argument, source) == text(right_argument, source)
        && IDEMPOTENT_BUILTINS.iter().any(|builtin| {
            is_builtin_named(model, left_function, source, builtin)
                && is_builtin_named(model, right_function, source, builtin)
        })
}

fn single_name_call(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if node.kind() != CALL {
        return None;
    }
    let arguments: Vec<Node<'_>> = named(node.child_by_field_name(ARGUMENTS_FIELD)?)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    match arguments.as_slice() {
        [argument] if argument.kind() == IDENTIFIER => {
            Some((node.child_by_field_name(FUNCTION_FIELD)?, *argument))
        }
        _ => None,
    }
}

// `is_literal_expr` de ruff: strings y bytes (sin f-strings), números, booleanos, `None` y `...`.
fn is_literal(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        STRING => !is_f_string(node, source),
        CONCATENATED_STRING => named(node).iter().all(|part| !is_f_string(*part, source)),
        TRUE | FALSE | NONE | ELLIPSIS => true,
        kind => NUMBERS.contains(&kind),
    }
}
