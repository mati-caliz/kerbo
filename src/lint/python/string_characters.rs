use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{INTERPOLATION, STRING, named};
use crate::rules::PythonRule;

const NEWLINE: u8 = b'\n';
const ZERO_WIDTH_SPACE: &str = "\u{200b}";
const INVALID: [(&[u8], PythonRule); 5] = [
    (b"\x08", PythonRule::PLE2510),
    (b"\x1a", PythonRule::PLE2512),
    (b"\x1b", PythonRule::PLE2513),
    (b"\x00", PythonRule::PLE2514),
    (ZERO_WIDTH_SPACE.as_bytes(), PythonRule::PLE2515),
];

// PLE2510, PLE2512, PLE2513, PLE2514 y PLE2515: un retroceso, SUB, ESC, NUL o espacio de ancho cero
// escrito tal cual adentro de un string (no en las expresiones de un f-string). Avisa en el carácter.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == STRING)
        .flat_map(|string| string_reports(*string, source))
        .collect()
}

fn string_reports(string: Node<'_>, source: &[u8]) -> Vec<Report> {
    let interpolations: Vec<(usize, usize)> = named(string)
        .into_iter()
        .filter(|child| child.kind() == INTERPOLATION)
        .map(|child| (child.start_byte(), child.end_byte()))
        .collect();
    let mut reports = Vec::new();
    let mut position = string.start_position();
    let mut offset = string.start_byte();
    while offset < string.end_byte() {
        let rest = source.get(offset..string.end_byte()).unwrap_or_default();
        let inside_interpolation = interpolations
            .iter()
            .any(|(start, end)| (*start..*end).contains(&offset));
        if !inside_interpolation
            && let Some((_, rule)) = INVALID.iter().find(|(character, _)| rest.starts_with(character))
        {
            reports.push(Report::at_point(*rule, position, ""));
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
