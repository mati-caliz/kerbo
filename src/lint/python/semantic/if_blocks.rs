use tree_sitter::Node;

use super::effects::contains_effect;
use super::model::Model;
use super::outdated_versions::outdated_version_blocks;
use super::qualified::{is_builtin_named, qualified_name};
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, COMMENT, COMPARISON, EXPRESSION_STATEMENT, IDENTIFIER, LEFT_FIELD, OPERATORS_FIELD,
    RIGHT_FIELD, SUBSCRIPT, SUBSCRIPT_FIELD, TYPE_FIELD, VALUE_FIELD, fields, named, text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::lint::python::fix_edits::has_comment_within;
use crate::lint::python::known_types::{Known, is_known};
use crate::rules::PythonRule;

const ELIF: &str = "elif_clause";
const ELSE: &str = "else_clause";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const VERSION_INFO: &str = "sys.version_info";
const VERSION_MAJOR: &str = "sys.version_info.major";
const LESS: &str = "<";
const GREATER_EQUAL: &str = ">=";
const IN: &str = "in";
const NOT_IN: [&str; 2] = ["not", "in"];
const MAX_LINE_LENGTH: usize = 88;
const GET_CALL: &str = ".get(";
const ARGUMENT_SEPARATOR: &str = ", ";
const ASSIGNMENT_OPERATOR: &str = " = ";

// UP036 (bloque para una versión de Python que ya no se soporta), PYI006 (`sys.version_info` comparado
// con algo que no es `<` ni `>=`) y SIM401 (`if k in d: x = d[k] else: x = default`).
pub fn if_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    let mut reports = Vec::new();
    let tests: Vec<Node<'_>> = statement
        .child_by_field_name(CONDITION_FIELD)
        .into_iter()
        .chain(
            fields(statement, ALTERNATIVE_FIELD)
                .into_iter()
                .filter(|clause| clause.kind() == ELIF)
                .filter_map(|clause| clause.child_by_field_name(CONDITION_FIELD)),
        )
        .collect();
    for test in &tests {
        reports.extend(bad_version_comparisons(model, source, *test));
    }
    reports.extend(outdated_version_blocks(model, source, statement, &tests));
    reports.extend(dict_get_block(model, source, statement));
    reports
}

pub struct Comparison<'tree> {
    pub left: Node<'tree>,
    pub operator: Vec<&'tree str>,
    pub right: Node<'tree>,
}

pub fn single_comparison<'tree>(test: Node<'tree>, source: &'tree [u8]) -> Option<Comparison<'tree>> {
    let test = unparenthesized(test);
    if test.kind() != COMPARISON {
        return None;
    }
    let operands: Vec<Node<'_>> = named(test)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let [left, right] = operands.as_slice() else {
        return None;
    };
    let mut cursor = test.walk();
    let operator: Vec<&str> = test
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    Some(Comparison {
        left: *left,
        operator,
        right: *right,
    })
}

pub fn operator_is(comparison: &Comparison<'_>, wanted: &str) -> bool {
    comparison.operator.as_slice() == [wanted]
}

// PYI006: también en cada parte de un `and`/`or`.
fn bad_version_comparisons(model: &Model<'_>, source: &[u8], test: Node<'_>) -> Vec<Report> {
    let test = unparenthesized(test);
    let parts = if test.kind() == BOOLEAN_OPERATOR {
        named(test)
    } else {
        vec![test]
    };
    parts
        .into_iter()
        .filter(|part| {
            single_comparison(*part, source).is_some_and(|comparison| {
                qualified_name(model, comparison.left, source).as_deref() == Some(VERSION_INFO)
                    && !operator_is(&comparison, GREATER_EQUAL)
                    && !operator_is(&comparison, LESS)
            })
        })
        .map(|part| Report::at(PythonRule::PYI006, unparenthesized(part), ""))
        .collect()
}

pub fn is_version_info(model: &Model<'_>, source: &[u8], left: Node<'_>) -> bool {
    let left = unparenthesized(left);
    let base = if left.kind() == SUBSCRIPT {
        left.child_by_field_name(VALUE_FIELD).unwrap_or(left)
    } else {
        left
    };
    qualified_name(model, base, source).as_deref() == Some(VERSION_INFO)
        || qualified_name(model, left, source).as_deref() == Some(VERSION_MAJOR)
}

fn single_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let statements: Vec<Node<'_>> = named(body?)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    match statements.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

fn plain_assignment(statement: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let assignment = named(statement)
        .into_iter()
        .find(|child| child.kind() == ASSIGNMENT)?;
    let value = assignment.child_by_field_name(RIGHT_FIELD)?;
    if assignment.child_by_field_name(TYPE_FIELD).is_some() || value.kind() == ASSIGNMENT {
        return None;
    }
    Some((assignment.child_by_field_name(LEFT_FIELD)?, value))
}

// SIM401 sobre un diccionario conocido, si el default no tiene efectos y `x = d.get(k, default)` entra
// en la línea.
fn dict_get_block(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Option<Report> {
    let alternatives = fields(statement, ALTERNATIVE_FIELD);
    let [alternative] = alternatives.as_slice() else {
        return None;
    };
    if alternative.kind() != ELSE {
        return None;
    }
    let (body_target, body_value) = plain_assignment(single_statement(
        statement.child_by_field_name(CONSEQUENCE_FIELD),
    )?)?;
    let (else_target, else_value) =
        plain_assignment(single_statement(alternative.child_by_field_name(BODY_FIELD))?)?;
    let comparison = single_comparison(statement.child_by_field_name(CONDITION_FIELD)?, source)?;
    let dictionary = unparenthesized(comparison.right);
    if dictionary.kind() != IDENTIFIER || !is_known(dictionary, Known::Dict, source) {
        return None;
    }
    let (target, value, default_target, default) = if operator_is(&comparison, IN) {
        (body_target, body_value, else_target, else_value)
    } else if is_not_in(&comparison) {
        (else_target, else_value, body_target, body_value)
    } else {
        return None;
    };
    let (key, subscripted) = subscript_parts(value)?;
    let matches = equivalent(key, comparison.left, source)
        && equivalent(target, default_target, source)
        && equivalent(dictionary, subscripted, source);
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    if !matches || contains_effect(default, source, &is_builtin) {
        return None;
    }
    let contents = [
        text(target, source),
        ASSIGNMENT_OPERATOR,
        &get_call(subscripted, comparison.left, default, source),
    ]
    .concat();
    let width = statement
        .start_position()
        .column
        .saturating_add(contents.chars().count());
    (width <= MAX_LINE_LENGTH).then(|| {
        Report::at(PythonRule::SIM401, statement, &contents)
            .with_fix(dict_get_fix(statement, &contents, source))
    })
}

// SIM401 en su forma de expresión: `d[k] if k in d else default`, sobre cualquier valor y sin mirar el
// largo de la línea.
pub fn dict_get_expression(model: &Model<'_>, source: &[u8], conditional: Node<'_>) -> Option<Report> {
    let parts: Vec<Node<'_>> = named(conditional)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let [body, test, orelse] = parts.as_slice() else {
        return None;
    };
    let comparison = single_comparison(*test, source)?;
    let (value, default) = if operator_is(&comparison, IN) {
        (*body, *orelse)
    } else if is_not_in(&comparison) {
        (*orelse, *body)
    } else {
        return None;
    };
    let (key, subscripted) = subscript_parts(value)?;
    let matches =
        equivalent(key, comparison.left, source) && equivalent(comparison.right, subscripted, source);
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    if !matches || contains_effect(default, source, &is_builtin) {
        return None;
    }
    let contents = get_call(subscripted, comparison.left, default, source);
    Some(
        Report::at(PythonRule::SIM401, conditional, &contents).with_fix(dict_get_fix(
            conditional,
            &contents,
            source,
        )),
    )
}

// El arreglo de SIM401: el bloque o la expresión pasan a la llamada a `get`; inseguro, y sin arreglo
// si hay comentarios.
fn dict_get_fix(node: Node<'_>, contents: &str, source: &[u8]) -> Option<Fix> {
    (!has_comment_within(node, node.start_byte(), node.end_byte()))
        .then(|| Fix::replace(node, contents, source).marked_unsafe())
}

fn is_not_in(comparison: &Comparison<'_>) -> bool {
    comparison
        .operator
        .iter()
        .flat_map(|operator| operator.split_whitespace())
        .eq(NOT_IN)
}

fn subscript_parts(value: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    let value = unparenthesized(value);
    if value.kind() != SUBSCRIPT {
        return None;
    }
    let mut cursor = value.walk();
    let keys: Vec<Node<'_>> = value
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    match keys.as_slice() {
        [key] => Some((*key, value.child_by_field_name(VALUE_FIELD)?)),
        _ => None,
    }
}

fn get_call(dictionary: Node<'_>, key: Node<'_>, default: Node<'_>, source: &[u8]) -> String {
    [
        text(dictionary, source),
        GET_CALL,
        text(key, source),
        ARGUMENT_SEPARATOR,
        text(default, source),
        ")",
    ]
    .concat()
}
