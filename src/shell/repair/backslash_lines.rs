use tree_sitter::Node;

use super::{Repaired, push};
use crate::shell::syntax::{COMMAND, COMMAND_NAME, WORD, descendants};

const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const SPACE: u8 = b' ';

// El lexer de tree-sitter-bash pega al comando de arriba una línea que empieza con `\` en la primera
// columna (`set -e⏎\. ./x`): arma una palabra que arranca en el salto de línea. Con un espacio adelante,
// la línea se lee sola.
pub fn glued_backslash_lines(root: Node<'_>, source: &[u8]) -> Vec<usize> {
    let mut starts: Vec<usize> = descendants(root)
        .into_iter()
        .filter(|node| {
            node.kind() == WORD
                && node
                    .parent()
                    .is_some_and(|parent| [COMMAND, COMMAND_NAME].contains(&parent.kind()))
        })
        .filter_map(|word| {
            let from = word.start_byte();
            let breaks = source
                .get(from..word.end_byte())?
                .iter()
                .take_while(|byte| matches!(**byte, LINE_FEED | CARRIAGE_RETURN))
                .count();
            (breaks > 0).then(|| from.saturating_add(breaks))
        })
        .collect();
    starts.sort_unstable();
    starts.dedup();
    starts
}

pub fn indent_lines(source: &[u8], line_starts: &[usize]) -> Repaired {
    let mut repaired = Vec::with_capacity(source.len().saturating_add(line_starts.len()));
    let mut origin = Vec::with_capacity(source.len().saturating_add(line_starts.len()));
    for (position, &byte) in source.iter().enumerate() {
        if line_starts.binary_search(&position).is_ok() {
            push(&mut repaired, &mut origin, SPACE, position);
        }
        push(&mut repaired, &mut origin, byte, position);
    }
    origin.push(source.len());
    Repaired {
        source: repaired,
        origin,
    }
}
