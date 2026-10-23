use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, COMMENT, CONCATENATED_STRING, FALSE, FUNCTION_FIELD, IDENTIFIER, NONE, STRING, TRUE, is_f_string,
    named, text, unparenthesized,
};
use crate::rules::PythonRule;

const ARGUMENTS_FIELD: &str = "arguments";
const NUMBERS: [&str; 2] = ["integer", "float"];
const ELLIPSIS: &str = "ellipsis";
const IDEMPOTENT_BUILTINS: [&str; 8] = ["id", "len", "type", "int", "bool", "str", "repr", "bytes"];

// PLR0124 (comparar algo consigo mismo) y PLR0133 (comparar dos constantes), por cada par de una
// comparación encadenada.
pub fn comparison_reports(model: &Model<'_>, source: &[u8], comparison: Node<'_>) -> Vec<Report> {
    let operands: Vec<Node<'_>> = named(comparison)
        .into_iter()
        .filter(|operand| operand.kind() != COMMENT)
        .map(unparenthesized)
        .collect();
    let mut reports = Vec::new();
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
