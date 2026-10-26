use tree_sitter::Node;

use super::Report;
use super::ast::{BLOCK, BODY_FIELD, COMMENT, named};
use super::fix_edits::{delete_between, point_of};
use crate::fix::{Fix, TextEdit};
use crate::rules::PythonRule;

const FOR: &str = "for_statement";
const LINE_FEED: char = '\n';
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
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| matches!(node.kind(), FOR | WHILE))
        .filter_map(|statement| {
            let else_clause = statement.child_by_field_name(ALTERNATIVE_FIELD)?;
            let body = statement.child_by_field_name(BODY_FIELD)?;
            (!exits_early(body)).then(|| {
                let fix = remove_else_fix(*statement, else_clause, source);
                Report::at(PythonRule::PLW0120, else_clause, "").with_fix(fix)
            })
        })
        .collect()
}

// El `remove_else` de ruff: con el cuerpo en la misma línea, se borra `else:` hasta la sentencia; si
// no, las líneas del cuerpo pasan a la sangría del loop (las vacías quedan vacías).
fn remove_else_fix(statement: Node<'_>, else_clause: Node<'_>, source: &[u8]) -> Option<Fix> {
    let body = else_clause.child_by_field_name(BODY_FIELD)?;
    let first = named(body).into_iter().find(|child| child.kind() != COMMENT)?;
    let Some(body_indentation) = indentation_of(first, source) else {
        return Some(delete_between(
            else_clause.start_byte(),
            first.start_byte(),
            source,
        ));
    };
    let loop_indentation = indentation_of(statement, source)?;
    let else_line_start = line_start(source, else_clause.start_byte());
    let body_start = line_end(source, else_clause.start_byte());
    let body_end = line_end(source, body.end_byte().saturating_sub(1));
    let lines = String::from_utf8_lossy(source.get(body_start..body_end)?).into_owned();
    let indented: String = lines
        .split_inclusive(LINE_FEED)
        .map(|line| {
            if line.trim().is_empty() {
                line.trim_start_matches([' ', '\t']).to_owned()
            } else if let Some(rest) = line.strip_prefix(body_indentation.as_str()) {
                format!("{loop_indentation}{rest}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    Some(Fix::from_edits(
        source,
        vec![TextEdit {
            start: point_of(source, else_line_start),
            end: point_of(source, body_end),
            replacement: indented,
        }],
    ))
}

fn indentation_of(node: Node<'_>, source: &[u8]) -> Option<String> {
    let start = line_start(source, node.start_byte());
    let before = source.get(start..node.start_byte())?;
    before
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t'))
        .then(|| String::from_utf8_lossy(before).into_owned())
}

fn line_start(source: &[u8], offset: usize) -> usize {
    source
        .get(..offset)
        .and_then(|before| before.iter().rposition(|byte| *byte == b'\n'))
        .map_or(0, |position| position.saturating_add(1))
}

// El final de la línea de `offset`, con su salto.
fn line_end(source: &[u8], offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|after| after.iter().position(|byte| *byte == b'\n'))
        .map_or(source.len(), |position| {
            offset.saturating_add(position).saturating_add(1)
        })
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
