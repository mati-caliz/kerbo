use tree_sitter::{Node, Point};

use crate::deploy::Violation;
use crate::fix::{Fix, TextEdit, TextPoint};

use super::syntax::{
    COMMAND as COMMAND_KIND, COMMENT as COMMENT_KIND, CONCATENATION, children, descendants, text,
};

const DOLLAR: u8 = b'$';
const ESCAPE: u8 = b'\\';
const SINGLE_QUOTE: u8 = b'\'';
const DOUBLE_QUOTE: u8 = b'"';
const OPEN_PARENTHESIS: u8 = b'(';
const CLOSE_PARENTHESIS: u8 = b')';
const OPEN_BRACE: u8 = b'{';
const CLOSE_BRACE: u8 = b'}';
const COMMENT: u8 = b'#';
const LINE_FEED: u8 = b'\n';
const UNDERSCORE: u8 = b'_';
const WORD_ENDS: &[u8] = b";|&()<>";
const DANGLING_DOLLAR: &str = "$";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Code { depth: usize },
    Quoted,
}

#[derive(Debug)]
pub struct Repaired {
    pub source: Vec<u8>,
    pub origin: Vec<usize>,
}

pub fn has_dangling_dollar(root: Node<'_>, source: &[u8]) -> bool {
    descendants(root).into_iter().any(|node| {
        node.kind() == CONCATENATION
            && children(node)
                .last()
                .is_some_and(|last| !last.is_named() && text(*last, source).trim() == DANGLING_DOLLAR)
    })
}

pub fn has_swallowed_lines(root: Node<'_>) -> bool {
    descendants(root).into_iter().any(|node| {
        node.kind() == COMMAND_KIND
            && children(node)
                .iter()
                .skip_while(|child| child.kind() != COMMENT_KIND)
                .any(|child| child.is_named() && child.kind() != COMMENT_KIND)
    })
}

pub fn brace_mid_word_expansions(source: &[u8]) -> Repaired {
    let mut repaired = Vec::with_capacity(source.len());
    let mut origin = Vec::with_capacity(source.len());
    let mut stack = vec![Context::Code { depth: 0 }];
    let mut index = 0;
    while let Some(&byte) = source.get(index) {
        let context = stack.last().copied().unwrap_or(Context::Code { depth: 0 });
        let name = (byte == DOLLAR && matches!(context, Context::Code { .. }))
            .then(|| braceable_name(source, index))
            .flatten();
        if let Some(end) = name {
            push(&mut repaired, &mut origin, DOLLAR, index);
            push(&mut repaired, &mut origin, OPEN_BRACE, index.saturating_add(1));
            for position in index.saturating_add(1)..end {
                push(
                    &mut repaired,
                    &mut origin,
                    source.get(position).copied().unwrap_or_default(),
                    position,
                );
            }
            push(&mut repaired, &mut origin, CLOSE_BRACE, end);
            index = end;
            continue;
        }
        let consumed = step(source, index, &mut stack);
        for position in index..index.saturating_add(consumed) {
            if let Some(&copied) = source.get(position) {
                push(&mut repaired, &mut origin, copied, position);
            }
        }
        index = index.saturating_add(consumed.max(1));
    }
    origin.push(source.len());
    Repaired {
        source: repaired,
        origin,
    }
}

fn push(repaired: &mut Vec<u8>, origin: &mut Vec<usize>, byte: u8, from: usize) {
    repaired.push(byte);
    origin.push(from);
}

fn braceable_name(source: &[u8], dollar: usize) -> Option<usize> {
    let previous = dollar.checked_sub(1).and_then(|before| source.get(before))?;
    if previous.is_ascii_whitespace() || *previous == ESCAPE || *previous == DOLLAR {
        return None;
    }
    let start = dollar.saturating_add(1);
    let first = *source.get(start)?;
    let end = if first.is_ascii_digit() {
        start.saturating_add(1)
    } else if first.is_ascii_alphabetic() || first == UNDERSCORE {
        let length = source
            .get(start..)?
            .iter()
            .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == UNDERSCORE)
            .count();
        start.saturating_add(length)
    } else {
        return None;
    };
    let next = *source.get(end)?;
    let continues = !next.is_ascii_whitespace() && !WORD_ENDS.contains(&next) && next != DOUBLE_QUOTE;
    continues.then_some(end)
}

fn step(source: &[u8], index: usize, stack: &mut Vec<Context>) -> usize {
    let byte = source.get(index).copied().unwrap_or_default();
    let next = source.get(index.saturating_add(1)).copied();
    let nested = stack.len() > 1;
    let Some(context) = stack.last_mut() else {
        return 1;
    };
    match (*context, byte) {
        (_, ESCAPE) => 2,
        (Context::Code { .. }, SINGLE_QUOTE) => skip_until(source, index, SINGLE_QUOTE),
        (Context::Code { .. }, COMMENT) if starts_word(source, index) => skip_until(source, index, LINE_FEED),
        (Context::Code { .. }, DOUBLE_QUOTE) => {
            stack.push(Context::Quoted);
            1
        }
        (Context::Quoted, DOUBLE_QUOTE) => {
            stack.pop();
            1
        }
        (_, DOLLAR) if next == Some(OPEN_PARENTHESIS) => {
            stack.push(Context::Code { depth: 1 });
            2
        }
        (Context::Code { depth }, OPEN_PARENTHESIS) => {
            *context = Context::Code {
                depth: depth.saturating_add(1),
            };
            1
        }
        (Context::Code { depth }, CLOSE_PARENTHESIS) if depth > 0 => {
            if depth == 1 && nested {
                stack.pop();
            } else {
                *context = Context::Code {
                    depth: depth.saturating_sub(1),
                };
            }
            1
        }
        _ => 1,
    }
}

fn starts_word(source: &[u8], index: usize) -> bool {
    index
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .is_none_or(u8::is_ascii_whitespace)
}

fn skip_until(source: &[u8], index: usize, end: u8) -> usize {
    source
        .get(index.saturating_add(1)..)
        .and_then(|rest| rest.iter().position(|byte| *byte == end))
        .map_or_else(
            || source.len().saturating_sub(index),
            |offset| offset.saturating_add(2),
        )
}

fn original_text_point(repaired: &Repaired, point: TextPoint, original: &[u8]) -> TextPoint {
    let byte = byte_of(&repaired.source, point);
    let original_byte = repaired.origin.get(byte).copied().unwrap_or(byte);
    TextPoint::from_point(point_of(original, original_byte))
}

pub fn restore(violation: Violation, repaired: &Repaired, original: &[u8]) -> Violation {
    let start = original_text_point(
        repaired,
        TextPoint {
            line: violation.line,
            column: violation.column,
        },
        original,
    );
    let subject = original_subject(repaired, &violation, original);
    let fix = violation.fix.map(|fix| {
        let edits = fix
            .edits
            .into_iter()
            .map(|edit| TextEdit {
                start: original_text_point(repaired, edit.start, original),
                end: original_text_point(repaired, edit.end, original),
                replacement: edit.replacement,
            })
            .collect();
        Fix::from_edits(original, edits)
    });
    Violation {
        line: start.line,
        column: start.column,
        subject,
        fix,
        ..violation
    }
}

fn original_subject(repaired: &Repaired, violation: &Violation, original: &[u8]) -> String {
    let start = byte_of(
        &repaired.source,
        TextPoint {
            line: violation.line,
            column: violation.column,
        },
    );
    let end = start.saturating_add(violation.subject.len());
    let shown = repaired.source.get(start..end).unwrap_or_default();
    if shown != violation.subject.as_bytes() {
        return violation.subject.clone();
    }
    let from = repaired.origin.get(start).copied().unwrap_or(start);
    let to = repaired.origin.get(end).copied().unwrap_or(end);
    original
        .get(from..to)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map_or_else(|| violation.subject.clone(), str::to_owned)
}

fn byte_of(source: &[u8], point: TextPoint) -> usize {
    let row = point.line.saturating_sub(1);
    let line_start = if row == 0 {
        0
    } else {
        source
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == LINE_FEED)
            .nth(row.saturating_sub(1))
            .map_or(source.len(), |(position, _)| position.saturating_add(1))
    };
    line_start.saturating_add(point.column.saturating_sub(1))
}

pub fn point_of(source: &[u8], byte: usize) -> Point {
    let before = source.get(..byte).unwrap_or(source);
    let line_start = before
        .iter()
        .rposition(|current| *current == LINE_FEED)
        .map_or(0, |position| position.saturating_add(1));
    Point {
        row: before
            .split(|current| *current == LINE_FEED)
            .count()
            .saturating_sub(1),
        column: byte.saturating_sub(line_start),
    }
}
