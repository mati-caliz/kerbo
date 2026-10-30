use tree_sitter::{Node, Point};

use super::Found;
use crate::rules::ShellRule;
use crate::shell::context::{Context, Position};
use crate::shell::syntax::{DESCRIPTOR_FIELD, DESTINATION_FIELD, FILE_REDIRECT, WORD, children, text};
use crate::shell::words::{literal, literal_or, parts};

const OUTPUT_AND_ERROR: &str = "&>";
const DUPLICATE_OUTPUT: &str = ">&";
const CLOSE_DESCRIPTOR: &str = "-";
const NETWORK_DEVICES: &[&str] = &["/dev/tcp", "/dev/udp"];
const GLOB_CHARACTERS: &[char] = &['*', '?'];
const OPEN_CLASS: char = '[';
const CLOSE_CLASS: char = ']';
const OPEN_NAME: u8 = b'{';
const CLOSE_NAME: u8 = b'}';
const NAME_UNDERSCORE: u8 = b'_';
const BRACES_WIDTH: usize = 2;

pub fn check(node: Node<'_>, context: &Context<'_, '_>, found: &mut Found<'_>) {
    if node.kind() != FILE_REDIRECT {
        return;
    }
    let source = context.source;
    let descriptor = node.child_by_field_name(DESCRIPTOR_FIELD);
    let Some(operator) = children(node)
        .into_iter()
        .find(|child| !child.is_named() && Some(*child) != descriptor)
    else {
        return;
    };
    let destination = node.child_by_field_name(DESTINATION_FIELD);
    let operator_text = text(operator, source);
    if operator_text == OUTPUT_AND_ERROR && !found.shell().is_busybox() {
        found.report(ShellRule::Sc3020, node, OUTPUT_AND_ERROR);
    }
    if operator_text == DUPLICATE_OUTPUT && descriptor.is_none() {
        check_duplicate_to_file(node, destination, source, found);
    }
    if let Some(descriptor) = descriptor
        && text(descriptor, source).len() > 1
        && text(descriptor, source)
            .chars()
            .all(|digit| digit.is_ascii_digit())
    {
        found.report(ShellRule::Sc3023, node, text(descriptor, source));
    }
    check_named_descriptor(node, source, found);
    let Some(destination) = destination else {
        return;
    };
    let file = literal_or(destination, source, "");
    if NETWORK_DEVICES.iter().any(|device| file.starts_with(device)) {
        found.report(ShellRule::Sc3025, operator, &file);
    }
    if is_glob(destination, source) {
        found.report(ShellRule::Sc3031, operator, text(destination, source));
    }
}

fn check_duplicate_to_file(
    node: Node<'_>,
    destination: Option<Node<'_>>,
    source: &[u8],
    found: &mut Found<'_>,
) {
    let Some(destination) = destination else {
        return;
    };
    let file = literal_or(destination, source, "");
    if file != CLOSE_DESCRIPTOR && !file.chars().all(|digit| digit.is_ascii_digit()) {
        found.report(ShellRule::Sc3021, node, text(destination, source));
    }
}

// tree-sitter-bash lee `{fd}>archivo` como un argumento `{fd}` pegado a la redirección.
fn check_named_descriptor(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let end = node.start_byte();
    if end == 0 || source.get(end.saturating_sub(1)) != Some(&CLOSE_NAME) {
        return;
    }
    let before = source.get(..end.saturating_sub(1)).unwrap_or_default();
    let name_length = before
        .iter()
        .rev()
        .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == NAME_UNDERSCORE)
        .count();
    let open = end.saturating_sub(name_length).saturating_sub(BRACES_WIDTH);
    let starts_word = open == 0
        || source
            .get(open.saturating_sub(1))
            .is_some_and(u8::is_ascii_whitespace);
    if name_length == 0 || source.get(open) != Some(&OPEN_NAME) || !starts_word {
        return;
    }
    let width = end.saturating_sub(open);
    let point = node.start_position();
    let position = Position {
        byte: open,
        point: Point {
            row: point.row,
            column: point.column.saturating_sub(width),
        },
    };
    found.report_at(ShellRule::Sc3022, position, "{fd}");
}

fn is_glob(destination: Node<'_>, source: &[u8]) -> bool {
    if literal(destination, source).is_some() {
        return false;
    }
    let unquoted: String = parts(destination)
        .into_iter()
        .filter(|part| part.kind() == WORD)
        .map(|part| text(part, source))
        .collect();
    unquoted.contains(GLOB_CHARACTERS)
        || unquoted
            .split_once(OPEN_CLASS)
            .is_some_and(|(_, after)| after.contains(CLOSE_CLASS))
}
