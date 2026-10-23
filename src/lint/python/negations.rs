use tree_sitter::Node;

use super::Report;
use super::ast::{
    CLASS, COMMENT, COMPARISON, FUNCTION, LAMBDA, NAME_FIELD, NOT_OPERATOR, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const ARGUMENT_FIELD: &str = "argument";
const OPERATORS_FIELD: &str = "operators";
const EQUAL: &str = "==";
const IF: &str = "if_statement";
const RAISE: &str = "raise_statement";
const CONSEQUENCE_FIELD: &str = "consequence";
const STATEMENT_SUFFIX: &str = "_statement";
const COMPARISON_DUNDERS: [&str; 6] = ["__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__"];
const SCOPE_OWNERS: [&str; 7] = [
    FUNCTION,
    CLASS,
    LAMBDA,
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];

// SIM201: `not a == b`, donde alcanza con `a != b`.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == NOT_OPERATOR && is_negated_equality(**node, source))
        .filter(|node| !is_exception_check(**node) && !in_comparison_dunder(**node, source))
        .map(|node| Report::at(PythonRule::SIM201, *node, ""))
        .collect()
}

fn is_negated_equality(negation: Node<'_>, source: &[u8]) -> bool {
    negation
        .child_by_field_name(ARGUMENT_FIELD)
        .map(unparenthesized)
        .filter(|operand| operand.kind() == COMPARISON)
        .is_some_and(|comparison| {
            let mut cursor = comparison.walk();
            let operators: Vec<&str> = comparison
                .children_by_field_name(OPERATORS_FIELD, &mut cursor)
                .map(|operator| text(operator, source))
                .collect();
            operators == [EQUAL]
        })
}

// `is_exception_check`: la condición de un `if` cuyo cuerpo es sólo un `raise`.
fn is_exception_check(negation: Node<'_>) -> bool {
    let statement = std::iter::successors(negation.parent(), Node::parent).find(|ancestor| {
        ancestor.kind().ends_with(STATEMENT_SUFFIX) || matches!(ancestor.kind(), FUNCTION | CLASS)
    });
    statement
        .filter(|statement| statement.kind() == IF)
        .and_then(|statement| statement.child_by_field_name(CONSEQUENCE_FIELD))
        .is_some_and(|body| {
            let statements: Vec<Node<'_>> = named(body)
                .into_iter()
                .filter(|child| child.kind() != COMMENT)
                .collect();
            matches!(statements.as_slice(), [only] if only.kind() == RAISE)
        })
}

fn in_comparison_dunder(negation: Node<'_>, source: &[u8]) -> bool {
    std::iter::successors(negation.parent(), Node::parent)
        .find(|ancestor| SCOPE_OWNERS.contains(&ancestor.kind()))
        .filter(|owner| owner.kind() == FUNCTION)
        .and_then(|function| function.child_by_field_name(NAME_FIELD))
        .is_some_and(|name| COMPARISON_DUNDERS.contains(&text(name, source)))
}
