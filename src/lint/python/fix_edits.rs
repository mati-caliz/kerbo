use tree_sitter::{Node, Point};

use super::ast::COMMENT;
use crate::fix::{Fix, TextEdit, TextPoint};

const COMMA: &str = ",";
const EMPTY_ARGUMENTS: &str = "()";
const LINE_FEED: u8 = b'\n';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parentheses {
    Remove,
    Preserve,
}

// El `remove_argument` de ruff: si hay argumentos después, desde el argumento hasta lo que sigue a su
// coma; si es el último, desde la coma anterior; si es el único, los paréntesis enteros (o `()`).
pub fn remove_argument(
    argument: Node<'_>,
    arguments: Node<'_>,
    parentheses: Parentheses,
    source: &[u8],
) -> Fix {
    if let Some(fix) = remove_listed(argument, arguments, source) {
        return fix;
    }
    match parentheses {
        Parentheses::Remove => Fix::replace(arguments, "", source),
        Parentheses::Preserve => Fix::replace(arguments, EMPTY_ARGUMENTS, source),
    }
}

// El `remove_member` de ruff: como `remove_argument`, pero si es el único elemento se borra sólo él.
pub fn remove_member(element: Node<'_>, container: Node<'_>, source: &[u8]) -> Fix {
    remove_listed(element, container, source).unwrap_or_else(|| Fix::replace(element, "", source))
}

// Los dos primeros casos de `remove_argument` y `remove_member`: hasta lo que sigue a la coma si hay
// elementos después, o desde la coma anterior si es el último.
fn remove_listed(argument: Node<'_>, arguments: Node<'_>, source: &[u8]) -> Option<Fix> {
    let children = all_children(arguments);
    let position = children
        .iter()
        .position(|child| child.id() == argument.id())
        .unwrap_or_default();
    let is_argument = |child: &Node<'_>| child.is_named() && child.kind() != COMMENT;
    let after = children.iter().skip(position.saturating_add(1));
    if children.iter().skip(position.saturating_add(1)).any(is_argument) {
        let mut rest = after.skip_while(|child| child.kind() != COMMA).skip(1);
        if let Some(next) = rest.next() {
            return Some(delete_between(argument.start_byte(), next.start_byte(), source));
        }
    }
    let previous = children.iter().take(position).rposition(is_argument);
    if let Some(comma) = previous.and_then(|previous| {
        children
            .iter()
            .skip(previous.saturating_add(1))
            .take(position.saturating_sub(previous))
            .find(|child| child.kind() == COMMA)
    }) {
        return Some(delete_between(comma.start_byte(), argument.end_byte(), source));
    }
    None
}

fn all_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

pub fn delete_between(start: usize, end: usize, source: &[u8]) -> Fix {
    Fix::from_edits(
        source,
        vec![TextEdit {
            start: point_of(source, start),
            end: point_of(source, end),
            replacement: String::new(),
        }],
    )
}

// Un comentario que cae en el rango, como `comment_ranges().intersects` de ruff: lo vuelve inseguro.
pub fn has_comment_within(node: Node<'_>, start: usize, end: usize) -> bool {
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.end_byte() <= start || current.start_byte() >= end {
            continue;
        }
        if current.kind() == COMMENT {
            return true;
        }
        pending.extend(all_children(current));
    }
    false
}

// Inseguro si algún comentario de `scope` cae en lo que el arreglo borra o reemplaza.
pub fn commented_unsafe(fix: Fix, scope: Node<'_>, source: &[u8]) -> Fix {
    let touched = fix.edits.iter().any(|edit| {
        let start = byte_of(source, edit.start);
        let end = byte_of(source, edit.end);
        has_comment_within(scope, start, end)
    });
    if touched { fix.marked_unsafe() } else { fix }
}

fn byte_of(source: &[u8], point: TextPoint) -> usize {
    let mut offset = 0;
    for _ in 1..point.line {
        let rest = source.get(offset..).unwrap_or_default();
        let Some(newline) = rest.iter().position(|byte| *byte == LINE_FEED) else {
            return source.len();
        };
        offset = offset.saturating_add(newline).saturating_add(1);
    }
    offset
        .saturating_add(point.column.saturating_sub(1))
        .min(source.len())
}

pub fn safe_unless_commented(fix: Fix, node: Node<'_>) -> Fix {
    let (start, end) = (node.start_byte(), node.end_byte());
    if has_comment_within(node, start, end) {
        fix.marked_unsafe()
    } else {
        fix
    }
}

// El `pad` de ruff: un espacio adelante o atrás si el reemplazo queda pegado a una letra.
pub fn pad(mut content: String, node: Node<'_>, source: &[u8]) -> String {
    if source.get(node.end_byte()).is_some_and(u8::is_ascii_alphabetic) {
        content.push(' ');
    }
    if node
        .start_byte()
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .is_some_and(u8::is_ascii_alphabetic)
    {
        content.insert(0, ' ');
    }
    content
}

// Como `remove_imports` de ruff, que reescribe la sentencia entera con libcst: el arreglo cubre toda
// la sentencia aunque cambie sólo la lista de nombres, y así desempata con otros igual que en ruff.
pub fn rewrite_names(statement: Node<'_>, entries: &[Node<'_>], names: &str, source: &[u8]) -> Option<Fix> {
    let first = entries.first()?;
    let last = entries.last()?;
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let replacement = format!(
        "{}{names}{}",
        slice(statement.start_byte(), first.start_byte()),
        slice(last.end_byte(), statement.end_byte())
    );
    Some(Fix::replace(statement, &replacement, source))
}

pub fn point_of(source: &[u8], offset: usize) -> TextPoint {
    let before = source.get(..offset).unwrap_or(source);
    let mut lines = before.split(|byte| *byte == LINE_FEED);
    let column = lines.next_back().map_or(0, <[u8]>::len);
    let row = lines.count();
    TextPoint::from_point(Point { row, column })
}
