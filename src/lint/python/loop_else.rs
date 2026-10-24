use tree_sitter::Node;

use super::Report;
use super::ast::{BLOCK, BODY_FIELD, named};
use crate::rules::PythonRule;

const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const BREAK: &str = "break_statement";
const ALTERNATIVE_FIELD: &str = "alternative";
const CASE_CLAUSE: &str = "case_clause";
const BRANCHING: [&str; 4] = [
    "if_statement",
    "with_statement",
    "try_statement",
    "match_statement",
];
const CLAUSES: [&str; 7] = [
    "elif_clause",
    "else_clause",
    "except_clause",
    "except_group_clause",
    "finally_clause",
    CASE_CLAUSE,
    BLOCK,
];

// PLW0120: un `else` en un `for` o `while` cuyo cuerpo nunca sale con `break`, así que el `else`
// corre siempre. Avisa en el `else`.
pub fn reports(nodes: &[Node<'_>]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| matches!(node.kind(), FOR | WHILE))
        .filter_map(|statement| {
            let else_clause = statement.child_by_field_name(ALTERNATIVE_FIELD)?;
            let body = statement.child_by_field_name(BODY_FIELD)?;
            (!exits_early(body)).then(|| Report::at(PythonRule::PLW0120, else_clause, ""))
        })
        .collect()
}

// `loop_exits_early`: algún `break` del cuerpo corta este loop: en el cuerpo, adentro de un `if`,
// `with`, `try` o `match`, o en el `else` de un loop anidado (el `break` de su cuerpo es suyo).
fn exits_early(block: Node<'_>) -> bool {
    named(block).into_iter().any(|statement| match statement.kind() {
        BREAK => true,
        FOR | WHILE => statement
            .child_by_field_name(ALTERNATIVE_FIELD)
            .is_some_and(|else_clause| blocks_of(else_clause).into_iter().any(exits_early)),
        kind if BRANCHING.contains(&kind) => blocks_of(statement).into_iter().any(exits_early),
        _ => false,
    })
}

// Los bloques de una sentencia compuesta y de sus cláusulas, sin entrar a sentencias anidadas.
fn blocks_of(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .flat_map(|child| match child.kind() {
            BLOCK if named(child).iter().any(|inner| inner.kind() == CASE_CLAUSE) => blocks_of(child),
            BLOCK => vec![child],
            kind if CLAUSES.contains(&kind) => blocks_of(child),
            _ => Vec::new(),
        })
        .collect()
}
