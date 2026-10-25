use tree_sitter::Node;

use super::Report;
use super::ast::COMMENT;
use crate::fix::Fix;
use crate::rules::PythonRule;

const OPENING: &str = "(";
const CLOSING: &str = ")";
const OPENINGS: [&str; 3] = ["(", "[", "{"];
const CLOSINGS: [&str; 3] = [")", "]", "}"];
const COMMA: &str = ",";
const YIELD: &str = "yield";
const LINE_CONTINUATION: &str = "line_continuation";

// UP034: `((x))` o `print((x))`, con un par de paréntesis de más. Calca el recorrido por tokens de
// ruff sobre las hojas del árbol, sin los comentarios ni las continuaciones de línea.
pub fn reports(root: Node<'_>, source: &[u8]) -> Vec<Report> {
    let tokens: Vec<Node<'_>> = leaves(root)
        .into_iter()
        .filter(|leaf| !matches!(leaf.kind(), COMMENT | LINE_CONTINUATION))
        .collect();
    let mut found = Vec::new();
    let mut position = 0;
    while let Some(token) = tokens.get(position) {
        position = position.saturating_add(1);
        if token.kind() != OPENING {
            continue;
        }
        if let Some((inner, closing)) = extraneous(&tokens, &mut position) {
            let contents = String::from_utf8_lossy(
                source
                    .get(inner.end_byte()..closing.start_byte())
                    .unwrap_or_default(),
            );
            let fix = Fix::replace_range(inner.start_position(), closing.end_position(), &contents, source);
            found.push(Report::at(PythonRule::UP034, inner, "").with_fix(Some(fix)));
        }
    }
    found
}

// `match_extraneous_parentheses`: avanza `position` como el iterador de ruff, también cuando no hay
// paréntesis de más.
fn extraneous<'tree>(tokens: &[Node<'tree>], position: &mut usize) -> Option<(Node<'tree>, Node<'tree>)> {
    let inner = next(tokens, position).filter(|token| token.kind() == OPENING)?;
    let mut depth = 1_usize;
    let mut empty = true;
    let closing = loop {
        let token = next(tokens, position)?;
        let kind = token.kind();
        if depth == 1 && matches!(kind, COMMA | YIELD) {
            return None;
        }
        if OPENINGS.contains(&kind) {
            depth = depth.saturating_add(1);
        } else if CLOSINGS.contains(&kind) {
            depth = depth.saturating_sub(1);
        }
        if depth == 0 {
            break token;
        }
        empty = false;
    };
    let after = next(tokens, position)?;
    (!empty && after.kind() == CLOSING).then_some((inner, closing))
}

fn next<'tree>(tokens: &[Node<'tree>], position: &mut usize) -> Option<Node<'tree>> {
    let token = tokens.get(*position).copied();
    *position = position.saturating_add(1);
    token
}

fn leaves(root: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.child_count() == 0 {
            found.push(node);
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return found;
            }
        }
    }
}
