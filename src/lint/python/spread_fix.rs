use tree_sitter::Node;

use super::ast::{COMMENT, named};
use super::fix_edits::point_of;
use crate::fix::{Fix, TextEdit};

const OPEN_PARENTHESIS: u8 = b'(';
const CLOSE_PARENTHESIS: u8 = b')';
const OPEN_BRACE: u8 = b'{';
const CLOSE_BRACE: u8 = b'}';
const COMMA: u8 = b',';
const HASH: u8 = b'#';
const BACKSLASH: u8 = b'\\';
const LINE_FEED: u8 = b'\n';

// El `unnecessary_spread_fix` de ruff, que trabaja sobre tokens: `{**a, **{"b": 1}}` pasa a
// `{**a, "b": 1}` borrando el `**`, los paréntesis y las llaves del de adentro, su coma final y, si
// estaba vacío, la coma que lo separaba del siguiente. Sin arreglo si aparece cualquier otra cosa.
pub fn unnecessary_spread_fix(splat: Node<'_>, inner: Node<'_>, source: &[u8]) -> Option<Fix> {
    let double_star = splat.child(0)?;
    let mut deletions = vec![(double_star.start_byte(), double_star.end_byte())];
    let mut open_parentheses = 0_usize;
    let mut cursor = double_star.end_byte();
    loop {
        let (position, byte) = next_token(source, cursor)?;
        match byte {
            OPEN_PARENTHESIS => open_parentheses = open_parentheses.saturating_add(1),
            OPEN_BRACE => {}
            _ => return None,
        }
        deletions.push((position, position.saturating_add(1)));
        cursor = position.saturating_add(1);
        if byte == OPEN_BRACE {
            break;
        }
    }
    let items: Vec<Node<'_>> = named(inner)
        .into_iter()
        .filter(|item| item.kind() != COMMENT)
        .collect();
    let empty = items.is_empty();
    cursor = items.last().map_or(cursor, Node::end_byte);
    let mut found_closing_brace = false;
    let mut found_dictionary_comma = false;
    while !(found_closing_brace && open_parentheses == 0 && (!empty || found_dictionary_comma)) {
        let Some((position, byte)) = next_token(source, cursor) else {
            break;
        };
        cursor = position.saturating_add(1);
        match byte {
            COMMA => {
                deletions.push((position, cursor));
                found_dictionary_comma |= found_closing_brace;
            }
            CLOSE_PARENTHESIS if found_closing_brace => {
                deletions.push((position, cursor));
                open_parentheses = open_parentheses.saturating_sub(1);
            }
            CLOSE_PARENTHESIS => {}
            CLOSE_BRACE if found_closing_brace => break,
            CLOSE_BRACE => {
                deletions.push((position, cursor));
                found_closing_brace = true;
            }
            _ => return None,
        }
    }
    let edits = deletions
        .into_iter()
        .map(|(start, end)| TextEdit {
            start: point_of(source, start),
            end: point_of(source, end),
            replacement: String::new(),
        })
        .collect();
    Some(Fix::from_edits(source, edits))
}

// El próximo carácter que no es espacio, comentario ni continuación de línea.
fn next_token(source: &[u8], mut position: usize) -> Option<(usize, u8)> {
    loop {
        let byte = *source.get(position)?;
        if byte.is_ascii_whitespace() {
            position = position.saturating_add(1);
        } else if byte == HASH {
            position = source
                .get(position..)?
                .iter()
                .position(|candidate| *candidate == LINE_FEED)
                .map_or(source.len(), |offset| position.saturating_add(offset));
        } else if byte == BACKSLASH && source.get(position.saturating_add(1)) == Some(&LINE_FEED) {
            position = position.saturating_add(2);
        } else {
            return Some((position, byte));
        }
    }
}
