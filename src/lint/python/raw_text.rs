use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{COMMENT, STRING, named, string_prefix, text};
use crate::rules::PythonRule;

const STRING_CONTENT: &str = "string_content";
const BACKSLASH: u8 = b'\\';
const RAW: [char; 2] = ['r', 'R'];
const VALID_ESCAPES: [char; 24] = [
    '\n', '\r', '\\', '\'', '"', 'a', 'b', 'f', 'n', 'r', 't', 'v', '0', '1', '2', '3', '4', '5', '6', '7',
    'x', 'N', 'u', 'U',
];
const SHEBANG: &str = "#!";
const BIDIRECTIONAL: [char; 11] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}', '\u{200F}', '\u{061C}',
];

// Chequeos sobre el texto tal cual: W605 (escape inválido en un string), EXE005 (shebang que no está
// al principio del archivo) y PLE2502 (caracteres de control bidireccionales).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            STRING => found.extend(invalid_escapes(*node, source)),
            COMMENT => found.extend(misplaced_shebang(*node, source)),
            _ => {}
        }
    }
    found.extend(bidirectional_lines(source));
    found
}

// W605: `"\."`, un escape que Python no conoce. Como ruff, `\\` consume la barra siguiente y cada tramo
// literal de un f-string se mira por separado.
fn invalid_escapes(string: Node<'_>, source: &[u8]) -> Vec<Report> {
    if string_prefix(string, source).contains(RAW) {
        return Vec::new();
    }
    named(string)
        .into_iter()
        .filter(|part| part.kind() == STRING_CONTENT)
        .flat_map(|content| invalid_escapes_in(content, source))
        .collect()
}

fn invalid_escapes_in(content: Node<'_>, source: &[u8]) -> Vec<Report> {
    let literal = text(content, source);
    let mut found = Vec::new();
    let mut previous: Option<usize> = None;
    for (index, byte) in literal.bytes().enumerate() {
        if byte != BACKSLASH {
            continue;
        }
        if previous.is_some_and(|previous| previous.saturating_add(1) == index) {
            previous = None;
            continue;
        }
        previous = Some(index);
        let Some(next) = literal
            .get(index.saturating_add(1)..)
            .and_then(|rest| rest.chars().next())
        else {
            continue;
        };
        if VALID_ESCAPES.contains(&next) {
            continue;
        }
        let before = literal.get(..index).unwrap_or_default();
        found.push(Report::at_point(
            PythonRule::W605,
            advanced(content.start_position(), before),
            &next.to_string(),
        ));
    }
    found
}

fn advanced(start: Point, before: &str) -> Point {
    match before.rsplit_once('\n') {
        Some((head, tail)) => Point {
            row: start
                .row
                .saturating_add(head.matches('\n').count())
                .saturating_add(1),
            column: tail.len(),
        },
        None => Point {
            row: start.row,
            column: start.column.saturating_add(before.len()),
        },
    }
}

// EXE005: `#!` en un comentario después de código; si antes sólo hay espacios es EXE004.
fn misplaced_shebang(comment: Node<'_>, source: &[u8]) -> Option<Report> {
    if !text(comment, source).starts_with(SHEBANG) {
        return None;
    }
    let before = source.get(..comment.start_byte())?;
    let only_whitespace = before
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\x0c' | b'\r' | b'\n'));
    (!only_whitespace).then(|| Report::at(PythonRule::EXE005, comment, ""))
}

fn bidirectional_lines(source: &[u8]) -> Vec<Report> {
    let Ok(contents) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    contents
        .split('\n')
        .enumerate()
        .filter(|(_, line)| line.contains(BIDIRECTIONAL))
        .map(|(row, _)| Report::at_point(PythonRule::PLE2502, Point { row, column: 0 }, ""))
        .collect()
}
