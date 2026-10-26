use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{INTERPOLATION, STRING, named, string_prefix};
use super::fix_edits::point_of;
use crate::fix::{Fix, TextEdit};
use crate::rules::PythonRule;

const NEWLINE: u8 = b'\n';
const ZERO_WIDTH_SPACE: &str = "\u{200b}";
const INVALID: [(&[u8], PythonRule, &str); 5] = [
    (b"\x08", PythonRule::PLE2510, "\\b"),
    (b"\x1a", PythonRule::PLE2512, "\\x1a"),
    (b"\x1b", PythonRule::PLE2513, "\\x1b"),
    (b"\x00", PythonRule::PLE2514, "\\0"),
    (ZERO_WIDTH_SPACE.as_bytes(), PythonRule::PLE2515, "\\u200b"),
];
const BACKSLASH: u8 = b'\\';
const RAW: [char; 2] = ['r', 'R'];

// PLE2510, PLE2512, PLE2513, PLE2514 y PLE2515: un retroceso, SUB, ESC, NUL o espacio de ancho cero
// escrito tal cual adentro de un string (no en las expresiones de un f-string). Avisa en el carácter.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == STRING)
        .flat_map(|string| string_reports(*string, source))
        .collect()
}

// Como ruff: una cantidad impar de barras justo antes escapa el carácter, y no se arregla.
fn is_escaped(source: &[u8], start: usize, offset: usize) -> bool {
    let before = source.get(start..offset).unwrap_or_default();
    before.iter().rev().take_while(|byte| **byte == BACKSLASH).count() % 2 == 1
}

fn string_reports(string: Node<'_>, source: &[u8]) -> Vec<Report> {
    let interpolations: Vec<(usize, usize)> = named(string)
        .into_iter()
        .filter(|child| child.kind() == INTERPOLATION)
        .map(|child| (child.start_byte(), child.end_byte()))
        .collect();
    let raw = string_prefix(string, source).contains(RAW);
    let mut reports = Vec::new();
    let mut position = string.start_position();
    let mut offset = string.start_byte();
    while offset < string.end_byte() {
        let rest = source.get(offset..string.end_byte()).unwrap_or_default();
        let inside_interpolation = interpolations
            .iter()
            .any(|(start, end)| (*start..*end).contains(&offset));
        if !inside_interpolation
            && let Some((character, rule, escape)) = INVALID
                .iter()
                .find(|(character, _, _)| rest.starts_with(character))
        {
            let fix = (!raw && !is_escaped(source, string.start_byte(), offset)).then(|| {
                let end = offset.saturating_add(character.len());
                Fix::from_edits(
                    source,
                    vec![TextEdit {
                        start: point_of(source, offset),
                        end: point_of(source, end),
                        replacement: (*escape).to_owned(),
                    }],
                )
            });
            reports.push(Report::at_point(*rule, position, "").with_fix(fix));
        }
        let Some(byte) = rest.first() else {
            break;
        };
        position = if *byte == NEWLINE {
            Point {
                row: position.row.saturating_add(1),
                column: 0,
            }
        } else {
            Point {
                row: position.row,
                column: position.column.saturating_add(1),
            }
        };
        offset = offset.saturating_add(1);
    }
    reports
}
