use tree_sitter::{Node, Point};

use super::context::Position;

const OPEN_NAME: u8 = b'{';
const CLOSE_NAME: u8 = b'}';
const NAME_UNDERSCORE: u8 = b'_';
const BRACES_WIDTH: usize = 2;

// tree-sitter-bash lee `{fd}>archivo` como un argumento `{fd}` pegado a la redirección.
pub fn named_descriptor(redirect: Node<'_>, source: &[u8]) -> Option<(String, Position)> {
    let end = redirect.start_byte();
    if end == 0 || source.get(end.saturating_sub(1)) != Some(&CLOSE_NAME) {
        return None;
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
        return None;
    }
    let name_start = open.saturating_add(1);
    let name_bytes = source.get(name_start..name_start.saturating_add(name_length))?;
    let name = std::str::from_utf8(name_bytes).ok()?.to_owned();
    let width = end.saturating_sub(open);
    let point = redirect.start_position();
    let position = Position {
        byte: open,
        point: Point {
            row: point.row,
            column: point.column.saturating_sub(width),
        },
    };
    Some((name, position))
}
