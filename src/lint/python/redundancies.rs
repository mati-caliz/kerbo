use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, CLASS, FUNCTION, IDENTIFIER, LAMBDA, LEFT_FIELD, RIGHT_FIELD, TUPLE, named, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const UNARY: &str = "unary_operator";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const DOUBLED_OPERATORS: [&str; 2] = ["+", "-"];
const TUPLES: [&str; 4] = [TUPLE, "pattern_list", "expression_list", "tuple_pattern"];

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            UNARY => doubled_unary(*node, source).into_iter().collect(),
            ASSIGNMENT => self_assignments(*node, source),
            _ => Vec::new(),
        })
        .collect()
}

// B002: `++x` o `--x`, que en Python no incrementa ni decrementa: son dos signos seguidos.
fn doubled_unary(operation: Node<'_>, source: &[u8]) -> Option<Report> {
    let operator = text(operation.child_by_field_name(OPERATOR_FIELD)?, source);
    let operand = operation
        .child_by_field_name(ARGUMENT_FIELD)
        .map(unparenthesized)
        .filter(|operand| operand.kind() == UNARY)?;
    let inner = text(operand.child_by_field_name(OPERATOR_FIELD)?, source);
    (DOUBLED_OPERATORS.contains(&operator) && operator == inner)
        .then(|| Report::at(PythonRule::B002, operation, ""))
}

// PLW0127: `x = x` (también adentro de tuplas y en asignaciones encadenadas), fuera del cuerpo de
// una clase, donde sí puede copiar un nombre de afuera.
fn self_assignments(assignment: Node<'_>, source: &[u8]) -> Vec<Report> {
    let is_chain_link = assignment
        .parent()
        .is_some_and(|parent| parent.kind() == ASSIGNMENT);
    if is_chain_link || is_in_class_scope(assignment) {
        return Vec::new();
    }
    let mut sides = Vec::new();
    let mut current = assignment;
    loop {
        sides.extend(current.child_by_field_name(LEFT_FIELD));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(right) if right.kind() == ASSIGNMENT => current = right,
            Some(right) => {
                sides.push(right);
                break;
            }
            None => break,
        }
    }
    let mut found = Vec::new();
    for (index, left) in sides.iter().enumerate() {
        for right in sides.iter().skip(index.saturating_add(1)) {
            same_names(*left, *right, source, &mut found);
        }
    }
    found
}

fn same_names<'tree>(left: Node<'tree>, right: Node<'tree>, source: &[u8], found: &mut Vec<Report>) {
    let (left, right) = (unparenthesized(left), unparenthesized(right));
    if TUPLES.contains(&left.kind()) && TUPLES.contains(&right.kind()) {
        let (lefts, rights) = (named(left), named(right));
        if lefts.len() == rights.len() {
            for (left_element, right_element) in lefts.into_iter().zip(rights) {
                same_names(left_element, right_element, source, found);
            }
        }
    } else if left.kind() == IDENTIFIER
        && right.kind() == IDENTIFIER
        && text(left, source) == text(right, source)
    {
        found.push(Report::at(PythonRule::PLW0127, left, ""));
    }
}

fn is_in_class_scope(node: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), CLASS | FUNCTION | LAMBDA))
        .is_some_and(|scope| scope.kind() == CLASS)
}
