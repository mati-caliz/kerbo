use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, text};
use super::fix_edits::delete_between;
use crate::fix::Fix;
use crate::rules::PythonRule;

const HASH: char = '#';
const PYTHON_WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];
const CARRIAGE_RETURN: char = '\r';
const NEWLINE: u8 = b'\n';
const CARRIAGE_RETURN_BYTE: u8 = b'\r';
const BACKSLASH: u8 = b'\\';
const MINIMUM_BLOCK: usize = 2;

// Un comentario y lo que ruff necesita de su línea.
struct Comment<'tree> {
    node: Node<'tree>,
    own_line: bool,
    column: usize,
    line: usize,
    empty: bool,
}

// PLR2044: un `#` sin nada después, salvo que sea parte de un bloque de comentarios con texto.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut comments: Vec<Comment<'_>> = nodes
        .iter()
        .filter(|node| node.kind() == COMMENT)
        .map(|node| describe(*node, source))
        .collect();
    comments.sort_by_key(|comment| comment.node.start_byte());
    let in_blocks = block_comments(&comments);
    comments
        .iter()
        .zip(in_blocks)
        .filter(|(comment, in_block)| comment.empty && !in_block)
        .map(|(comment, _)| {
            Report::at(PythonRule::PLR2044, comment.node, "")
                .with_fix(Some(empty_comment_fix(comment.node, source)))
        })
        .collect()
}

// Como ruff: con código antes en la línea, desde el final de ese código; detrás de una continuación
// `\`, desde el `#`; si no, la línea entera.
fn empty_comment_fix(comment: Node<'_>, source: &[u8]) -> Fix {
    let start = comment.start_byte();
    let line_start = source
        .get(..start)
        .and_then(|before| before.iter().rposition(|byte| *byte == NEWLINE))
        .map_or(0, |newline| newline.saturating_add(1));
    let line_end = source
        .get(start..)
        .and_then(|rest| rest.iter().position(|byte| *byte == NEWLINE))
        .map_or(source.len(), |newline| start.saturating_add(newline));
    let line_end = if source.get(line_end.saturating_sub(1)) == Some(&CARRIAGE_RETURN_BYTE) {
        line_end.saturating_sub(1)
    } else {
        line_end
    };
    let before = source.get(line_start..start).unwrap_or_default();
    let code_end = before
        .iter()
        .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\x0c' | b'#'))
        .map(|index| line_start.saturating_add(index).saturating_add(1));
    let continued = line_start > 1 && source.get(line_start.saturating_sub(2)) == Some(&BACKSLASH);
    match code_end {
        Some(code_end) => delete_between(code_end, line_end, source),
        None if continued => delete_between(start, line_end, source),
        None => {
            let next_line = source
                .get(start..)
                .and_then(|rest| rest.iter().position(|byte| *byte == NEWLINE))
                .map_or(source.len(), |newline| {
                    start.saturating_add(newline).saturating_add(1)
                });
            delete_between(line_start, next_line, source)
        }
    }
}

fn describe<'tree>(node: Node<'tree>, source: &[u8]) -> Comment<'tree> {
    let start = node.start_byte();
    let line_start = source
        .get(..start)
        .and_then(|before| before.iter().rposition(|byte| *byte == NEWLINE))
        .map_or(0, |newline| newline.saturating_add(1));
    let indentation = source.get(line_start..start).unwrap_or_default();
    let content = text(node, source).trim_end_matches(CARRIAGE_RETURN);
    Comment {
        node,
        own_line: indentation
            .iter()
            .all(|byte| PYTHON_WHITESPACE.contains(&char::from(*byte))),
        column: start.saturating_sub(line_start),
        line: node.start_position().row,
        empty: content.strip_prefix(HASH).is_some_and(|rest| {
            rest.chars()
                .all(|character| PYTHON_WHITESPACE.contains(&character))
        }),
    }
}

// `CommentRanges::block_comments`: comentarios propios de línea, seguidos, con la misma sangría,
// de a dos o más y con alguno que tenga texto.
fn block_comments(comments: &[Comment<'_>]) -> Vec<bool> {
    let mut in_block = vec![false; comments.len()];
    let mut current: Vec<usize> = Vec::new();
    let mut current_column: Option<usize> = None;
    let mut non_empty = false;
    let mut previous_line: Option<usize> = None;
    for (index, comment) in comments.iter().enumerate() {
        let follows = previous_line.is_some_and(|line| line.saturating_add(1) == comment.line);
        let continues = comment.own_line && follows && current_column == Some(comment.column);
        if !continues {
            flush(&current, non_empty, &mut in_block);
            current.clear();
            non_empty = false;
            current_column = comment.own_line.then_some(comment.column);
        }
        if comment.own_line {
            current.push(index);
            non_empty |= !comment.empty;
        }
        previous_line = Some(comment.line);
    }
    flush(&current, non_empty, &mut in_block);
    in_block
}

fn flush(block: &[usize], non_empty: bool, in_block: &mut [bool]) {
    if block.len() >= MINIMUM_BLOCK && non_empty {
        for index in block {
            if let Some(flag) = in_block.get_mut(*index) {
                *flag = true;
            }
        }
    }
}
