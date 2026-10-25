use tree_sitter::{Node, Point};

use super::ast::{BLOCK, COMMENT, named};
use crate::fix::{Fix, TextEdit, TextPoint};

const PASS: &str = "pass";
const SEMICOLON: u8 = b';';
const LINE_FEED: u8 = b'\n';
const BACKSLASH: u8 = b'\\';
const INLINE_SPACE: &[u8] = b" \t\x0c";

// El `delete_stmt` de ruff: `pass` si es la única sentencia del bloque; si comparte la línea con un
// `;`, sólo ella y su separador; si no, las líneas enteras, con su comentario. Queda aislada con las
// demás del mismo bloque.
pub fn delete_statement(statement: Node<'_>, source: &[u8]) -> Fix {
    let fix = if is_lone_child(statement) {
        Fix::replace(statement, PASS, source)
    } else if let Some(end) = trailing_semicolon(statement, source) {
        byte_range_fix(statement.start_byte(), end, source)
    } else if let Some(start) = preceding_semicolon(statement, source) {
        byte_range_fix(start, statement.end_byte(), source)
    } else {
        full_lines(statement, source)
    };
    fix.isolated(isolation_group(statement))
}

// El grupo de aislamiento de ruff: la sentencia que contiene al bloque; en el módulo, ninguno.
pub fn isolation_group(statement: Node<'_>) -> Option<usize> {
    statement
        .parent()
        .filter(|parent| parent.kind() == BLOCK)
        .and_then(|block| block.parent())
        .map(|owner| owner.id())
}

fn is_lone_child(statement: Node<'_>) -> bool {
    statement.parent().is_some_and(|parent| {
        parent.kind() == BLOCK
            && named(parent)
                .into_iter()
                .filter(|child| child.kind() != COMMENT)
                .count()
                == 1
    })
}

// Después de la sentencia, saltando espacios y continuaciones: si hay un `;`, hasta donde empieza lo
// que sigue en la línea.
fn trailing_semicolon(statement: Node<'_>, source: &[u8]) -> Option<usize> {
    let mut index = skip_forward(source, statement.end_byte());
    if source.get(index) != Some(&SEMICOLON) {
        return None;
    }
    index = index.saturating_add(1);
    while source.get(index).is_some_and(|byte| INLINE_SPACE.contains(byte)) {
        index = index.saturating_add(1);
    }
    Some(index)
}

fn preceding_semicolon(statement: Node<'_>, source: &[u8]) -> Option<usize> {
    let mut index = statement.start_byte();
    while index > 0 {
        let previous = *source.get(index.saturating_sub(1))?;
        if INLINE_SPACE.contains(&previous) {
            index = index.saturating_sub(1);
        } else if previous == SEMICOLON {
            return Some(index.saturating_sub(1));
        } else {
            return None;
        }
    }
    None
}

fn skip_forward(source: &[u8], mut index: usize) -> usize {
    loop {
        match source.get(index) {
            Some(byte) if INLINE_SPACE.contains(byte) => index = index.saturating_add(1),
            Some(&BACKSLASH) if source.get(index.saturating_add(1)) == Some(&LINE_FEED) => {
                index = index.saturating_add(2);
            }
            _ => return index,
        }
    }
}

fn full_lines(statement: Node<'_>, source: &[u8]) -> Fix {
    let after_line = source
        .get(statement.end_byte()..)
        .and_then(|rest| rest.iter().position(|byte| *byte == LINE_FEED))
        .map_or(source.len(), |offset| {
            statement.end_byte().saturating_add(offset).saturating_add(1)
        });
    let start = Point {
        row: statement.start_position().row,
        column: 0,
    };
    byte_points_fix(start, after_line, source)
}

fn byte_range_fix(start: usize, end: usize, source: &[u8]) -> Fix {
    Fix::from_edits(
        source,
        vec![TextEdit {
            start: point_of(source, start),
            end: point_of(source, end),
            replacement: String::new(),
        }],
    )
}

fn byte_points_fix(start: Point, end: usize, source: &[u8]) -> Fix {
    Fix::from_edits(
        source,
        vec![TextEdit {
            start: TextPoint::from_point(start),
            end: point_of(source, end),
            replacement: String::new(),
        }],
    )
}

fn point_of(source: &[u8], offset: usize) -> TextPoint {
    let before = source.get(..offset).unwrap_or(source);
    let mut lines = before.split(|byte| *byte == LINE_FEED);
    let column = lines.next_back().map_or(0, <[u8]>::len);
    let row = lines.count();
    TextPoint::from_point(Point { row, column })
}
