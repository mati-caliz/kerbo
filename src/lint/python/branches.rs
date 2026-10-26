use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, PARENTHESIZED, STRING, string_prefix, text};
use super::escapes::text_value;
use super::fix_edits::point_of;
use crate::fix::{Fix, TextEdit};
use crate::rules::PythonRule;

const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const CONDITION_FIELD: &str = "condition";
const ELSE: &str = "else_clause";
const NEWLINE: u8 = b'\n';
const COLON: &str = ":";
const LOOSE: [&str; 3] = ["lambda", "named_expression", "conditional_expression"];

// SIM114: dos ramas seguidas de un `if`/`elif` con el mismo cuerpo, que se pueden juntar con `or`.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == IF)
        .flat_map(|statement| same_arms(*statement, source))
        .collect()
}

// El `merge_branches` de ruff: la condición de la segunda rama se suma con `or` a la de la primera
// (entre paréntesis si es una lambda, un `:=` o un `if`) y se borran las líneas de la segunda.
fn merge_branches_fix(current: Node<'_>, following: Node<'_>, source: &[u8]) -> Option<Fix> {
    let current_test = current.child_by_field_name(CONDITION_FIELD)?;
    let following_test = following.child_by_field_name(CONDITION_FIELD)?;
    let colon = (0..current.child_count())
        .filter_map(|index| current.child(index))
        .find(|child| child.kind() == COLON && child.start_byte() >= current_test.end_byte())?;
    let current_end = current.child_by_field_name(CONSEQUENCE_FIELD)?.end_byte();
    let following_end = following.child_by_field_name(CONSEQUENCE_FIELD)?.end_byte();
    let wrap = |test: Node<'_>| {
        let written = text(test, source);
        if LOOSE.contains(&test.kind()) {
            format!("({written})")
        } else {
            written.to_owned()
        }
    };
    let mut edits = vec![TextEdit {
        start: point_of(source, full_line_end(source, current_end)),
        end: point_of(source, full_line_end(source, following_end)),
        replacement: String::new(),
    }];
    if LOOSE.contains(&current_test.kind()) {
        edits.push(TextEdit {
            start: point_of(source, current_test.start_byte()),
            end: point_of(source, current_test.end_byte()),
            replacement: wrap(current_test),
        });
    }
    edits.push(TextEdit {
        start: point_of(source, colon.start_byte()),
        end: point_of(source, colon.start_byte()),
        replacement: format!(" or {}", wrap(following_test)),
    });
    Some(Fix::from_edits(source, edits))
}

fn full_line_end(source: &[u8], offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|after| after.iter().position(|byte| *byte == NEWLINE))
        .map_or(source.len(), |position| {
            offset.saturating_add(position).saturating_add(1)
        })
}

fn same_arms(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut cursor = statement.walk();
    let branches: Vec<Node<'_>> = std::iter::once(statement)
        .chain(
            statement
                .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
                .filter(|clause| clause.kind() == ELIF),
        )
        .collect();
    branches
        .windows(2)
        .filter_map(|pair| match pair {
            [current, following] if same_body(*current, *following, source) => Some(
                Report::at(PythonRule::SIM114, *current, "")
                    .with_fix(merge_branches_fix(*current, *following, source)),
            ),
            _ => None,
        })
        .collect()
}

fn same_body(first: Node<'_>, second: Node<'_>, source: &[u8]) -> bool {
    let (Some(first_body), Some(second_body)) = (
        first.child_by_field_name(CONSEQUENCE_FIELD),
        second.child_by_field_name(CONSEQUENCE_FIELD),
    ) else {
        return false;
    };
    equivalent(first_body, second_body, source)
        && body_comments(first, source) == body_comments(second, source)
}

// Los comentarios desde la línea siguiente a la condición hasta el final de la última línea del
// cuerpo, como `body_range` de ruff; tree-sitter deja algunos colgando de la rama y no del bloque.
fn body_comments<'source>(branch: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    let Some(body) = branch.child_by_field_name(CONSEQUENCE_FIELD) else {
        return Vec::new();
    };
    let start = line_end(
        source,
        branch
            .child_by_field_name(CONDITION_FIELD)
            .map_or_else(|| branch.start_byte(), |condition| condition.end_byte()),
    );
    let end = line_end(source, body.end_byte());
    let mut found = Vec::new();
    let mut pending = vec![branch];
    while let Some(node) = pending.pop() {
        if node.kind() == COMMENT && node.start_byte() >= start && node.end_byte() <= end {
            found.push((node.start_byte(), text(node, source)));
        }
        let mut cursor = node.walk();
        pending.extend(
            node.children(&mut cursor)
                .filter(|child| child.kind() != ELIF && child.kind() != ELSE),
        );
    }
    found.sort_unstable();
    found.into_iter().map(|(_, comment)| comment).collect()
}

// `ComparableStmt` de ruff, aproximado sobre el árbol: mismos nodos y tokens, sin comentarios ni
// paréntesis de agrupación, y los strings por su valor.
pub fn equivalent(first: Node<'_>, second: Node<'_>, source: &[u8]) -> bool {
    let first = without_parentheses(first);
    let second = without_parentheses(second);
    if first.kind() != second.kind() {
        return false;
    }
    if first.kind() == STRING {
        let value = |string: Node<'_>| {
            (
                string_prefix(string, source).to_ascii_lowercase(),
                text_value(string, source),
            )
        };
        let (first_value, second_value) = (value(first), value(second));
        return if first_value.1.is_some() {
            first_value == second_value
        } else {
            text(first, source) == text(second, source)
        };
    }
    let first_children = significant_children(first);
    let second_children = significant_children(second);
    if first_children.is_empty() && second_children.is_empty() {
        return text(first, source) == text(second, source);
    }
    first_children.len() == second_children.len()
        && first_children
            .iter()
            .zip(&second_children)
            .all(|(left, right)| equivalent(*left, *right, source))
}

fn significant_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn without_parentheses(node: Node<'_>) -> Node<'_> {
    if node.kind() == PARENTHESIZED {
        let mut cursor = node.walk();
        let inner: Vec<Node<'_>> = node
            .named_children(&mut cursor)
            .filter(|child| child.kind() != COMMENT)
            .collect();
        if let [single] = inner.as_slice() {
            return without_parentheses(*single);
        }
    }
    node
}

fn line_end(source: &[u8], offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|rest| rest.iter().position(|byte| *byte == NEWLINE))
        .map_or(source.len(), |distance| offset.saturating_add(distance))
}
