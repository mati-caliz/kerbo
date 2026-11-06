use tree_sitter::{Node, Point};

use crate::rules::ShellRule;
use crate::shell::context::{Findings, Position};
use crate::shell::syntax::{ASSIGNMENT, NAME_FIELD, RAW_STRING, VALUE_FIELD, text};

const FIELD_SEPARATOR_VARIABLE: &str = "IFS";
const SPACES: &[u8] = b" \t";
const END_OF_COMMAND: &[u8] = b"\r\n;&|)#";
const SINGLE_QUOTE: char = '\'';
const ESCAPE: u8 = b'\\';
const ESCAPED_CLOSING_QUOTE: &str = "\\'";

// Avisos que ShellCheck da mientras parsea y no cortan la lectura.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() == ASSIGNMENT {
        check_space_after_equals(node, source, findings);
    } else if node.kind() == RAW_STRING {
        check_escaped_quote(node, source, findings);
    }
}

// SC1007: `a= b`, salvo `IFS= read`.
fn check_space_after_equals(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    if assignment.child_by_field_name(VALUE_FIELD).is_some() {
        return;
    }
    let name = assignment
        .child_by_field_name(NAME_FIELD)
        .map_or("", |name| text(name, source));
    let after = source.get(assignment.end_byte()..).unwrap_or_default();
    let spaces = after.iter().take_while(|byte| SPACES.contains(byte)).count();
    let next = after.get(spaces);
    let ends_command = next.is_none_or(|byte| END_OF_COMMAND.contains(byte));
    if name != FIELD_SEPARATOR_VARIABLE && spaces > 0 && !ends_command {
        let position = Position {
            byte: assignment.end_byte(),
            point: assignment.end_position(),
        };
        findings.report_at(ShellRule::Sc1007, position, name);
    }
}

// SC1003: `'it\'s'`, donde la barra no escapa la comilla que cierra.
fn check_escaped_quote(string: Node<'_>, source: &[u8], findings: &mut Findings) {
    let raw = text(string, source);
    let follows_escape = string
        .start_byte()
        .checked_sub(1)
        .and_then(|previous| source.get(previous))
        == Some(&ESCAPE);
    if follows_escape || !raw.starts_with(SINGLE_QUOTE) || !raw.ends_with(ESCAPED_CLOSING_QUOTE) {
        return;
    }
    let offset = raw.len().saturating_sub(ESCAPED_CLOSING_QUOTE.len());
    let before = raw.get(..offset).unwrap_or_default();
    let lines = before.matches('\n').count();
    let start = string.start_position();
    let point = before.rfind('\n').map_or_else(
        || Point {
            row: start.row,
            column: start.column.saturating_add(offset),
        },
        |line_feed| Point {
            row: start.row.saturating_add(lines),
            column: before.len().saturating_sub(line_feed).saturating_sub(1),
        },
    );
    let position = Position {
        byte: string.start_byte().saturating_add(offset),
        point,
    };
    findings.report_at(ShellRule::Sc1003, position, raw);
}
