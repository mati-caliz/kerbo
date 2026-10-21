use tree_sitter::Node;

use super::Report;
use super::ast::{CLASS, FUNCTION, LAMBDA};
use crate::rules::PythonRule;

const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const RETURN: &str = "return_statement";
const YIELD: &str = "yield";
const AWAIT: &str = "await";
const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const ELSE: &str = "else_clause";
const COMPREHENSIONS: [&str; 4] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];

pub fn reports(nodes: &[Node<'_>]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.is_named())
        .filter_map(|node| match node.kind() {
            BREAK => outside_loop(*node).then(|| Report::at(PythonRule::F701, *node, "")),
            CONTINUE => outside_loop(*node).then(|| Report::at(PythonRule::F702, *node, "")),
            RETURN => outside_function(*node).then(|| Report::at(PythonRule::F706, *node, "")),
            YIELD | AWAIT => outside_function(*node).then(|| Report::at(PythonRule::F704, *node, "")),
            _ => None,
        })
        .collect()
}

fn outside_loop(statement: Node<'_>) -> bool {
    let mut child = statement;
    for parent in std::iter::successors(statement.parent(), Node::parent) {
        match parent.kind() {
            FOR | WHILE if child.kind() != ELSE => return false,
            FUNCTION | CLASS => return true,
            _ => {}
        }
        child = parent;
    }
    true
}

fn outside_function(node: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| {
            matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA) || COMPREHENSIONS.contains(&ancestor.kind())
        })
        .is_none_or(|scope| scope.kind() == CLASS)
}
