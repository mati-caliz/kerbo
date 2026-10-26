use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::effects::contains_effect;
use super::if_blocks::single_comparison;
use super::model::Model;
use super::qualified::is_builtin_named;
use super::tail_call_fixes::identity_check_fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENT_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, NONE, NOT_OPERATOR, RIGHT_FIELD, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const IDENTITY_OPERATORS: [&[&str]; 5] = [&["=="], &["!="], &["is"], &["is", "not"], &["is not"]];
const TYPE_BUILTIN: &str = "type";
const NOT_EQUAL: &str = "!=";
const NOT_KEYWORD: &str = "not";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const AND: &str = "and";
const OR: &str = "or";
const OPERATOR_FIELD: &str = "operator";

// FURB169: `type(x) == type(None)` (o `is`, `!=`, `is not`), que es `x is None`. Avisa en la
// comparación.
pub fn type_none_comparison(model: &Model<'_>, source: &[u8], comparison: Node<'_>) -> Option<Report> {
    let parts = single_comparison(comparison, source)?;
    if !IDENTITY_OPERATORS.contains(&parts.operator.as_slice()) {
        return None;
    }
    let (left, right) = (
        type_argument(model, source, parts.left)?,
        type_argument(model, source, parts.right)?,
    );
    (left.kind() == NONE || right.kind() == NONE).then(|| {
        let other = if left.kind() == NONE { right } else { left };
        let negated = parts
            .operator
            .iter()
            .any(|operator| *operator == NOT_EQUAL || operator.contains(NOT_KEYWORD));
        let fix = identity_check_fix(comparison, other, negated, source);
        Report::at(PythonRule::FURB169, comparison, "").with_fix(Some(fix))
    })
}

// El `x` de `type(x)` con el builtin.
fn type_argument<'tree>(model: &Model<'_>, source: &[u8], node: Node<'tree>) -> Option<Node<'tree>> {
    let node = unparenthesized(node);
    let given = arguments(node);
    let function = node
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|_| node.kind() == CALL)?;
    match (given.positional.as_slice(), given.keywords.as_slice()) {
        ([only], []) if is_builtin_named(model, function, source, TYPE_BUILTIN) => {
            Some(unparenthesized(*only))
        }
        _ => None,
    }
}

// SIM220 y SIM221: `a and not a` (siempre falso) o `a or not a` (siempre verdadero). Un aviso por cada
// par, en la cadena entera.
pub fn negated_operands(model: &Model<'_>, source: &[u8], operation: Node<'_>) -> Vec<Report> {
    let operator = operation
        .child_by_field_name(OPERATOR_FIELD)
        .map(|operator| text(operator, source));
    let rule = match operator {
        Some(AND) => PythonRule::SIM220,
        Some(OR) => PythonRule::SIM221,
        _ => return Vec::new(),
    };
    let nested = operation.parent().is_some_and(|parent| {
        parent.kind() == BOOLEAN_OPERATOR
            && parent
                .child_by_field_name(OPERATOR_FIELD)
                .map(|inner| text(inner, source))
                == operator
    });
    if nested {
        return Vec::new();
    }
    let mut operands = Vec::new();
    flatten(operation, source, operator, &mut operands);
    let (negated, plain): (Vec<Node<'_>>, Vec<Node<'_>>) = operands
        .into_iter()
        .partition(|operand| operand.kind() == NOT_OPERATOR);
    let negated: Vec<Node<'_>> = negated
        .into_iter()
        .filter_map(|operand| operand.child_by_field_name(ARGUMENT_FIELD).map(unparenthesized))
        .collect();
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    if negated.is_empty() || contains_effect(operation, source, &is_builtin) {
        return Vec::new();
    }
    let mut reports = Vec::new();
    for inner in &negated {
        for other in &plain {
            if inner.kind() == IDENTIFIER
                && other.kind() == IDENTIFIER
                && text(*inner, source) == text(*other, source)
            {
                reports.push(Report::at(rule, operation, ""));
            }
        }
    }
    reports
}

fn flatten<'tree>(node: Node<'tree>, source: &[u8], operator: Option<&str>, operands: &mut Vec<Node<'tree>>) {
    for side in [LEFT_FIELD, RIGHT_FIELD]
        .into_iter()
        .filter_map(|field| node.child_by_field_name(field))
    {
        let same = side.kind() == BOOLEAN_OPERATOR
            && side
                .child_by_field_name(OPERATOR_FIELD)
                .map(|inner| text(inner, source))
                == operator;
        if same {
            flatten(side, source, operator, operands);
        } else {
            operands.push(unparenthesized(side));
        }
    }
}
