use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, OBJECT_FIELD, RIGHT_FIELD,
    text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::rules::PythonRule;

pub const BOOLEAN_OPERATOR: &str = "boolean_operator";
const OPERATOR_FIELD: &str = "operator";
const OR: &str = "or";
const ISINSTANCE: &str = "isinstance";
const AFFIX_METHODS: [&str; 2] = ["startswith", "endswith"];
const LIST_SPLAT: &str = "list_splat";

// SIM101: `isinstance(x, A) or isinstance(x, B)`, que se escribe `isinstance(x, (A, B))`. Como ruff,
// junta sólo las llamadas seguidas sobre el mismo valor y avisa una vez por grupo en el `or` entero.
pub fn duplicate_isinstance_reports(model: &Model<'_>, source: &[u8], operation: Node<'_>) -> Vec<Report> {
    if !is_or(operation, source) || is_inner_operand(operation, source) {
        return Vec::new();
    }
    let mut operands = Vec::new();
    flatten(operation, source, &mut operands);
    let mut reports = repeated_affix_checks(&operands, source);
    let mut previous: Option<Node<'_>> = None;
    let mut run = 0_usize;
    for operand in operands {
        let target = isinstance_target(model, source, operand);
        let continues = matches!(
            (previous, target),
            (Some(last), Some(current)) if equivalent(last, current, source)
        );
        if continues {
            run = run.saturating_add(1);
            if run == 2 {
                reports.push(Report::at(PythonRule::SIM101, operation, ""));
            }
        } else {
            run = usize::from(target.is_some());
        }
        previous = target;
    }
    reports
}

// PIE810: `s.startswith("a") or s.startswith("b")`, que es `s.startswith(("a", "b"))`. Avisa una vez
// por variable y método en el `or` entero.
fn repeated_affix_checks(operands: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let keys: Vec<(&str, &str)> = operands
        .iter()
        .filter_map(|operand| affix_check(unparenthesized(*operand), source))
        .collect();
    let mut reported: Vec<(&str, &str)> = Vec::new();
    let mut reports = Vec::new();
    for key in &keys {
        if !reported.contains(key) && keys.iter().filter(|other| *other == key).count() > 1 {
            reported.push(*key);
            if let Some(operation) = operands.first().and_then(|first| outermost_or(*first, source)) {
                reports.push(Report::at(PythonRule::PIE810, operation, ""));
            }
        }
    }
    reports
}

fn outermost_or<'tree>(operand: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(operand.parent(), Node::parent)
        .take_while(|ancestor| is_or(*ancestor, source))
        .last()
}

fn affix_check<'source>(call: Node<'_>, source: &'source [u8]) -> Option<(&'source str, &'source str)> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| call.kind() == CALL && function.kind() == ATTRIBUTE)?;
    let method = text(function.child_by_field_name(ATTRIBUTE_FIELD)?, source);
    let receiver = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|receiver| receiver.kind() == IDENTIFIER)?;
    let given = arguments(call);
    let single = matches!(given.positional.as_slice(), [only] if only.kind() != LIST_SPLAT)
        && given.keywords.is_empty();
    (AFFIX_METHODS.contains(&method) && single).then(|| (text(receiver, source), method))
}

fn is_or(operation: Node<'_>, source: &[u8]) -> bool {
    operation.kind() == BOOLEAN_OPERATOR
        && operation
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == OR)
}

// Un `a or b` sin paréntesis adentro de otro `or` es parte del mismo `BoolOp` de ruff.
fn is_inner_operand(operation: Node<'_>, source: &[u8]) -> bool {
    operation.parent().is_some_and(|parent| is_or(parent, source))
}

fn flatten<'tree>(node: Node<'tree>, source: &[u8], operands: &mut Vec<Node<'tree>>) {
    if is_or(node, source) {
        for side in [LEFT_FIELD, RIGHT_FIELD] {
            if let Some(child) = node.child_by_field_name(side) {
                flatten(child, source, operands);
            }
        }
    } else {
        operands.push(node);
    }
}

fn isinstance_target<'tree>(model: &Model<'_>, source: &[u8], operand: Node<'tree>) -> Option<Node<'tree>> {
    let call = unparenthesized(operand);
    if call.kind() != CALL
        || !call
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(model, function, source, ISINSTANCE))
    {
        return None;
    }
    let given = arguments(call);
    match (given.positional.as_slice(), given.keywords.is_empty()) {
        ([target, _], true) => Some(*target),
        _ => None,
    }
}
