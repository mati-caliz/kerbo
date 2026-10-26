use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{COMMENT, STRING, named, string_prefix, text};
use crate::fix::Fix;
use crate::rules::PythonRule;

const STRING_CONTENT: &str = "string_content";
const INTERPOLATION: &str = "interpolation";
const FORMAT_SPECIFIER: &str = "format_specifier";
const BACKSLASH: u8 = b'\\';
const RAW: [char; 2] = ['r', 'R'];
const UNICODE: [char; 2] = ['u', 'U'];
const RAW_PREFIX: &str = "r";
const VALID_ESCAPES: [char; 24] = [
    '\n', '\r', '\\', '\'', '"', 'a', 'b', 'f', 'n', 'r', 't', 'v', '0', '1', '2', '3', '4', '5', '6', '7',
    'x', 'N', 'u', 'U',
];
const SHEBANG: &str = "#!";
const BIDIRECTIONAL: [char; 11] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}', '\u{200F}', '\u{061C}',
];

// Chequeos sobre el texto tal cual: W605 (escape inválido en un string), EXE004 y EXE005 (shebang que
// no está al principio del archivo) y PLE2502 (caracteres de control bidireccionales).
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
// literal de un f-string se mira por separado. El arreglo de ruff depende de toda la parte: si tiene
// algún escape válido duplica la barra de cada inválido; si no, la vuelve raw (`u` pasa a `r`).
fn invalid_escapes(string: Node<'_>, source: &[u8]) -> Vec<Report> {
    let prefix = string_prefix(string, source);
    if prefix.contains(RAW) {
        return Vec::new();
    }
    let mut contains_valid_escape = false;
    let mut invalid: Vec<InvalidEscape> = Vec::new();
    for (start, literal) in literal_pieces(string, source) {
        contains_valid_escape |= scan_escapes(start, literal, &mut invalid);
    }
    invalid
        .into_iter()
        .map(|escape| {
            let fix = if contains_valid_escape {
                Fix::insert_at(escape.after_backslash, "\\", source)
            } else {
                raw_prefix_fix(string, prefix, source)
            };
            Report::at_point(PythonRule::W605, escape.start, &escape.character.to_string())
                .with_fix(Some(fix))
        })
        .collect()
}

struct InvalidEscape {
    start: Point,
    after_backslash: Point,
    character: char,
}

fn raw_prefix_fix(string: Node<'_>, prefix: &str, source: &[u8]) -> Fix {
    let start = string.start_position();
    if prefix.contains(UNICODE) {
        let after_prefix = Point {
            row: start.row,
            column: start.column.saturating_add(1),
        };
        return Fix::replace_range(start, after_prefix, RAW_PREFIX, source);
    }
    let glued_to_name = string
        .start_byte()
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    let inserted = if glued_to_name {
        format!(" {RAW_PREFIX}")
    } else {
        RAW_PREFIX.to_owned()
    };
    Fix::insert_at(start, &inserted, source)
}

// Los tramos literales del string: su contenido y, en un f-string, el texto del formato de cada
// interpolación (`{x:\>10}`) sin las interpolaciones que tenga adentro.
fn literal_pieces<'source>(string: Node<'_>, source: &'source [u8]) -> Vec<(Point, &'source str)> {
    let mut pieces = Vec::new();
    for part in named(string) {
        if part.kind() == STRING_CONTENT {
            pieces.push((part.start_position(), text(part, source)));
        } else if part.kind() == INTERPOLATION {
            for specifier in named(part)
                .into_iter()
                .filter(|child| child.kind() == FORMAT_SPECIFIER)
            {
                pieces.extend(specifier_pieces(specifier, source));
            }
        }
    }
    pieces
}

fn specifier_pieces<'source>(specifier: Node<'_>, source: &'source [u8]) -> Vec<(Point, &'source str)> {
    let whole = text(specifier, source);
    let mut pieces = Vec::new();
    let mut cursor = 0;
    let nested_ranges = named(specifier).into_iter().map(|nested| {
        let start = nested.start_byte().saturating_sub(specifier.start_byte());
        let end = nested.end_byte().saturating_sub(specifier.start_byte());
        (start, end)
    });
    for (start, end) in nested_ranges.chain(std::iter::once((whole.len(), whole.len()))) {
        if let Some(piece) = whole.get(cursor..start).filter(|piece| !piece.is_empty()) {
            let before = whole.get(..cursor).unwrap_or_default();
            pieces.push((advanced(specifier.start_position(), before), piece));
        }
        cursor = end;
    }
    pieces
}

// Junta los escapes inválidos del tramo y dice si vio alguno válido.
fn scan_escapes(piece_start: Point, literal: &str, invalid: &mut Vec<InvalidEscape>) -> bool {
    let mut contains_valid_escape = false;
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
            contains_valid_escape = true;
            continue;
        }
        let before = literal.get(..index).unwrap_or_default();
        let start = advanced(piece_start, before);
        invalid.push(InvalidEscape {
            start,
            after_backslash: Point {
                row: start.row,
                column: start.column.saturating_add(1),
            },
            character: next,
        });
    }
    contains_valid_escape
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

// EXE005: `#!` en un comentario después de código. EXE004: si antes sólo hay espacios, que avisa al
// principio del archivo.
fn misplaced_shebang(comment: Node<'_>, source: &[u8]) -> Option<Report> {
    if !text(comment, source).starts_with(SHEBANG) {
        return None;
    }
    let before = source.get(..comment.start_byte())?;
    let only_whitespace = before
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\x0c' | b'\r' | b'\n'));
    if !only_whitespace {
        return Some(Report::at(PythonRule::EXE005, comment, ""));
    }
    if before.is_empty() {
        return None;
    }
    let fix = Fix::replace_range(Point::default(), comment.start_position(), "", source);
    let fix = if before.contains(&b'\n') {
        fix.marked_unsafe()
    } else {
        fix
    };
    Some(Report::at_point(PythonRule::EXE004, Point::default(), "").with_fix(Some(fix)))
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
