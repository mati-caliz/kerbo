use tree_sitter::Node;

use crate::lint::python::ast::{COMMENT, NAME_FIELD, fields, text};

const CLOSING: &str = ")";
const COMMA: &str = ",";
const LINE_FEED: char = '\n';
const COMMENT_START: char = '#';

// El `retain_imports` y el `remove_imports` de ruff con libcst: la sentencia sólo con esos nombres,
// cada uno con su coma y lo que la sigue. El último que queda se lleva la coma final de la sentencia,
// salvo que la suya tenga un comentario.
pub fn rebuilt_import(statement: Node<'_>, kept: &[Node<'_>], source: &[u8]) -> Option<String> {
    let entries = fields(statement, NAME_FIELD);
    let first = *entries.first()?;
    let last = *entries.last()?;
    let slice =
        |start: usize, end: usize| String::from_utf8_lossy(source.get(start..end).unwrap_or_default());
    let (trailing_comma, closing) = statement_tail(statement, last, source);
    let mut content = slice(statement.start_byte(), first.start_byte()).into_owned();
    let retained: Vec<(usize, Node<'_>)> = entries
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, entry)| kept.iter().any(|moved| moved.id() == entry.id()))
        .collect();
    let (&(last_index, last_kept), leading) = retained.split_last()?;
    for (index, entry) in leading {
        let next = entries.get(index.saturating_add(1))?;
        content.push_str(text(*entry, source));
        content.push_str(&slice(entry.end_byte(), next.start_byte()));
    }
    content.push_str(text(last_kept, source));
    let own_separator = entries
        .get(last_index.saturating_add(1))
        .map(|next| slice(last_kept.end_byte(), next.start_byte()));
    match own_separator {
        Some(separator) if has_comment_on_first_line(&separator) => content.push_str(&separator),
        _ => content.push_str(&trailing_comma),
    }
    content.push_str(&closing);
    Some(content)
}

// Lo que sigue al último nombre: la coma final con el espacio hasta el paréntesis (de la coma, en
// libcst) y el resto hasta el final de la sentencia.
fn statement_tail(statement: Node<'_>, last: Node<'_>, source: &[u8]) -> (String, String) {
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let mut cursor = statement.walk();
    let after: Vec<Node<'_>> = statement
        .children(&mut cursor)
        .filter(|child| child.start_byte() >= last.end_byte() && child.kind() != COMMENT)
        .collect();
    let comma_and_closing = after
        .first()
        .filter(|comma| comma.kind() == COMMA)
        .zip(after.get(1).filter(|closing| closing.kind() == CLOSING));
    match comma_and_closing {
        Some((_, closing)) => (
            slice(last.end_byte(), closing.start_byte()),
            slice(closing.start_byte(), statement.end_byte()),
        ),
        None => (String::new(), slice(last.end_byte(), statement.end_byte())),
    }
}

fn has_comment_on_first_line(separator: &str) -> bool {
    separator
        .split(LINE_FEED)
        .next()
        .is_some_and(|line| line.contains(COMMENT_START))
}
