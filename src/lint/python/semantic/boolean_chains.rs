use tree_sitter::Node;

use super::effects::contains_effect;
use super::model::Model;
use super::qualified::is_builtin_named;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    COMMENT, LEFT_FIELD, RIGHT_FIELD, SUBSCRIPT, VALUE_FIELD, named, text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::lint::python::fix_edits::commented_unsafe;
use crate::rules::PythonRule;

const BOOLEAN_OPERATOR: &str = "boolean_operator";
const PARENTHESIZED_EXPRESSION: &str = "parenthesized_expression";
const OPEN_PARENTHESIS: &str = "(";
const CLOSE_PARENTHESIS: &str = ")";
const COMPARISON: &str = "comparison_operator";
const OPERATOR_FIELD: &str = "operator";
const OPERATORS_FIELD: &str = "operators";
const AND: &str = "and";
const IN: &str = "in";
const ASCENDING: [&str; 2] = ["<", "<="];
const DESCENDING: [&str; 2] = [">", ">="];

// RUF019 (`"k" in d and d["k"]`, que es `d.get("k")`) y PLR1716 (`a < b and b < c`, que es
// `a < b < c`), sobre un `and` entero.
pub fn boolean_chain_reports(model: &Model<'_>, source: &[u8], operation: Node<'_>) -> Vec<Report> {
    if !is_and(operation, source) || operation.parent().is_some_and(|parent| is_and(parent, source)) {
        return Vec::new();
    }
    let mut operands = Vec::new();
    flatten(operation, source, &mut operands);
    let mut reports = Vec::new();
    if let [left, right] = operands.as_slice()
        && is_unnecessary_key_check(model, source, *left, *right)
    {
        let fix = key_check_fix(operation, *right, source);
        reports.push(Report::at(PythonRule::RUF019, operation, "").with_fix(fix));
    }
    for pair in operands.windows(2) {
        if let [left, right] = pair
            && is_chainable(source, *left, *right)
        {
            reports.push(
                Report::at(PythonRule::PLR1716, *left, "")
                    .with_fix(chained_comparison_fix(*left, *right, source)),
            );
        }
    }
    reports
}

fn is_and(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == BOOLEAN_OPERATOR
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == AND)
}

fn flatten<'tree>(node: Node<'tree>, source: &[u8], operands: &mut Vec<Node<'tree>>) {
    if is_and(node, source) {
        for side in [LEFT_FIELD, RIGHT_FIELD] {
            if let Some(child) = node.child_by_field_name(side) {
                flatten(child, source, operands);
            }
        }
    } else {
        operands.push(unparenthesized(node));
    }
}

fn comparison_parts<'tree>(
    node: Node<'tree>,
    source: &'tree [u8],
) -> Option<(Vec<Node<'tree>>, Vec<&'tree str>)> {
    if node.kind() != COMPARISON {
        return None;
    }
    let operands: Vec<Node<'tree>> = named(node)
        .into_iter()
        .filter(|operand| operand.kind() != COMMENT)
        .collect();
    let mut cursor = node.walk();
    let operators = node
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    Some((operands, operators))
}

// El arreglo de RUF019: `d.get(k)` con el diccionario y la clave del subíndice, tal como están
// escritos; inseguro si hay comentarios.
fn key_check_fix(operation: Node<'_>, subscript: Node<'_>, source: &[u8]) -> Option<Fix> {
    let dictionary = subscript.child_by_field_name(VALUE_FIELD)?;
    let key = named(subscript).into_iter().nth(1)?;
    let replacement = format!("{}.get({})", text(dictionary, source), text(key, source));
    Some(commented_unsafe(
        Fix::replace(operation, &replacement, source),
        operation,
        source,
    ))
}

fn is_unnecessary_key_check(model: &Model<'_>, source: &[u8], left: Node<'_>, right: Node<'_>) -> bool {
    let Some((operands, operators)) = comparison_parts(left, source) else {
        return false;
    };
    let ([key, dictionary], [IN]) = (operands.as_slice(), operators.as_slice()) else {
        return false;
    };
    let indexes = right.kind() == SUBSCRIPT
        && right
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| equivalent(value, *dictionary, source))
        && matches!(named(right).as_slice(), [_, index] if equivalent(*index, *key, source));
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    indexes
        && !contains_effect(*dictionary, source, &is_builtin)
        && !contains_effect(*key, source, &is_builtin)
}

// El arreglo de PLR1716: borra desde el lado derecho de la primera comparación hasta el izquierdo de
// la segunda, dejando el primero, y equilibra los paréntesis que envolvían a cada una.
fn chained_comparison_fix(left: Node<'_>, right: Node<'_>, source: &[u8]) -> Option<Fix> {
    let left_comparison = unparenthesized(left);
    let right_comparison = unparenthesized(right);
    let middle = *comparison_operands(left_comparison).last()?;
    let next = *comparison_operands(right_comparison).first()?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(middle.start_position()),
        end: TextPoint::from_point(next.end_position()),
        replacement: text(middle, source).to_owned(),
    }];
    let (left_depth, right_depth) = (parenthesis_depth(left), parenthesis_depth(right));
    if left_depth < right_depth {
        edits.push(TextEdit {
            start: TextPoint::from_point(left_comparison.start_position()),
            end: TextPoint::from_point(left_comparison.start_position()),
            replacement: OPEN_PARENTHESIS.repeat(right_depth.saturating_sub(left_depth)),
        });
    } else if left_depth > right_depth {
        edits.push(TextEdit {
            start: TextPoint::from_point(right_comparison.end_position()),
            end: TextPoint::from_point(right_comparison.end_position()),
            replacement: CLOSE_PARENTHESIS.repeat(left_depth.saturating_sub(right_depth)),
        });
    }
    Some(Fix::from_edits(source, edits))
}

fn comparison_operands(comparison: Node<'_>) -> Vec<Node<'_>> {
    named(comparison)
        .into_iter()
        .filter(|operand| operand.kind() != COMMENT)
        .collect()
}

fn parenthesis_depth(mut node: Node<'_>) -> usize {
    let mut depth = 0_usize;
    while node.kind() == PARENTHESIZED_EXPRESSION {
        depth = depth.saturating_add(1);
        match named(node).into_iter().find(|inner| inner.kind() != COMMENT) {
            Some(inner) => node = inner,
            None => break,
        }
    }
    depth
}

fn is_chainable(source: &[u8], left: Node<'_>, right: Node<'_>) -> bool {
    let (Some((left_operands, left_operators)), Some((right_operands, right_operators))) =
        (comparison_parts(left, source), comparison_parts(right, source))
    else {
        return false;
    };
    let ([_, middle], [left_operator], [first, _], [right_operator]) = (
        left_operands.as_slice(),
        left_operators.as_slice(),
        right_operands.as_slice(),
        right_operators.as_slice(),
    ) else {
        return false;
    };
    let same_direction = (ASCENDING.contains(left_operator) && ASCENDING.contains(right_operator))
        || (DESCENDING.contains(left_operator) && DESCENDING.contains(right_operator));
    same_direction && equivalent(*middle, *first, source)
}
