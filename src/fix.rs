mod hunk;

use tree_sitter::{Node, Point};

use crate::syntax::one_based;

const HORIZONTAL_SPACE: &[u8] = b" \t";
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextPoint {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start: TextPoint,
    pub end: TextPoint,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub edits: Vec<TextEdit>,
    pub hunk: String,
}

impl TextPoint {
    pub const fn from_point(point: Point) -> Self {
        Self {
            line: one_based(point.row),
            column: one_based(point.column),
        }
    }

    const fn line_start(row: usize) -> Self {
        Self::from_point(Point { row, column: 0 })
    }
}

impl Fix {
    fn single(source: &[u8], start: TextPoint, end: TextPoint, replacement: &str) -> Self {
        let edits = vec![TextEdit {
            start,
            end,
            replacement: replacement.to_owned(),
        }];
        Self {
            hunk: hunk::unified(source, &edits).unwrap_or_default(),
            edits,
        }
    }

    pub fn insert_before(node: Node<'_>, text: &str, source: &[u8]) -> Self {
        Self::insert_at(node.start_position(), text, source)
    }

    pub fn insert_at(point: Point, text: &str, source: &[u8]) -> Self {
        let at = TextPoint::from_point(point);
        Self::single(source, at, at, text)
    }

    pub fn remove(node: Node<'_>, source: &[u8]) -> Self {
        let first_row = node.start_position().row;
        let last_row = node.end_position().row;
        if is_alone_on_its_lines(node, source) {
            return Self::single(
                source,
                TextPoint::line_start(first_row),
                TextPoint::line_start(last_row.saturating_add(1)),
                "",
            );
        }
        let trailing = source
            .get(node.end_byte()..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| HORIZONTAL_SPACE.contains(byte))
            .count();
        let end = node.end_position();
        let end = Point {
            row: end.row,
            column: end.column.saturating_add(trailing),
        };
        Self::single(
            source,
            TextPoint::from_point(node.start_position()),
            TextPoint::from_point(end),
            "",
        )
    }
}

pub fn apply(edits: &[TextEdit], text: &str) -> Option<String> {
    let mut ordered: Vec<&TextEdit> = edits.iter().collect();
    ordered.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    let mut fixed = text.to_owned();
    for edit in ordered {
        let start = byte_offset(&fixed, edit.start)?;
        let end = byte_offset(&fixed, edit.end)?;
        fixed.replace_range(start..end, &edit.replacement);
    }
    Some(fixed)
}

fn is_alone_on_its_lines(node: Node<'_>, source: &[u8]) -> bool {
    let is_blank = |bytes: &[u8]| {
        bytes
            .iter()
            .all(|byte| HORIZONTAL_SPACE.contains(byte) || *byte == CARRIAGE_RETURN)
    };
    let before = source.get(..node.start_byte()).unwrap_or_default();
    let line_start = before
        .iter()
        .rposition(|&byte| byte == LINE_FEED)
        .map_or(0, |index| index.saturating_add(1));
    let after = source.get(node.end_byte()..).unwrap_or_default();
    let line_end = after
        .iter()
        .position(|&byte| byte == LINE_FEED)
        .unwrap_or(after.len());
    is_blank(before.get(line_start..).unwrap_or_default())
        && is_blank(after.get(..line_end).unwrap_or_default())
}

fn byte_offset(text: &str, point: TextPoint) -> Option<usize> {
    let row = point.line.checked_sub(1)?;
    let column = point.column.checked_sub(1)?;
    let line_start = match row.checked_sub(1) {
        None => 0,
        Some(previous) => match text.match_indices(char::from(LINE_FEED)).nth(previous) {
            Some((index, _)) => index.saturating_add(1),
            None if column == 0 && text.matches(char::from(LINE_FEED)).count() == previous => text.len(),
            None => return None,
        },
    };
    let offset = line_start.saturating_add(column);
    (offset <= text.len() && text.is_char_boundary(offset)).then_some(offset)
}

#[cfg(test)]
mod tests;
