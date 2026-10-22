use std::collections::HashMap;

use tree_sitter::Node;

use super::super::ast::{Comment, CommentGroup};
use super::super::position::{LineTable, Pos, signed};
use super::COMMENT;

const LINE_COMMENT_PREFIX: &str = "//";
const CARRIAGE_RETURN: char = '\r';
const CLOSING_SLASH: &str = "\r/";
const NEWLINE: u8 = b'\n';
const STRING_KINDS: [&str; 2] = ["interpreted_string_literal", "raw_string_literal"];
const TERMINATOR_KINDS: [&str; 2] = ["\n", "\0"];
const CLOSING_KINDS: [&str; 2] = [")", "}"];
const SEMICOLON: &str = ";";
const SEMICOLON_TRIGGERS: [&str; 26] = [
    "identifier",
    "type_identifier",
    "field_identifier",
    "package_identifier",
    "label_name",
    "blank_identifier",
    "int_literal",
    "float_literal",
    "imaginary_literal",
    "rune_literal",
    "interpreted_string_literal",
    "raw_string_literal",
    "nil",
    "true",
    "false",
    "iota",
    "break",
    "continue",
    "fallthrough",
    "fallthrough_statement",
    "return",
    "++",
    "--",
    ")",
    "]",
    "}",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Semicolon,
    InsertedSemicolon,
    Closing,
    Other,
    Comment(usize),
}

#[derive(Debug, Clone, Copy)]
struct Scanned {
    offset: usize,
    end: usize,
    line: i64,
    kind: Kind,
    inserts_semicolon: bool,
}

// Los comentarios del archivo agrupados como los agrupa go/parser, con lo necesario para saber qué
// grupo es el comentario de línea de una declaración.
#[derive(Debug)]
pub struct Comments {
    pub groups: Vec<CommentGroup>,
    scanned: Vec<Scanned>,
    group_at: HashMap<usize, usize>,
}

pub fn scan(root: Node<'_>, source: &str, lines: &LineTable) -> Comments {
    let mut comments = Vec::new();
    let mut items = Vec::new();
    collect(root, source, &mut comments, &mut items);
    let scanned = with_semicolons(&items, &comments, source.as_bytes(), lines);
    let mut grouper = Grouper {
        scanned: &scanned,
        comments: &comments,
        lines,
        groups: Vec::new(),
        group_at: HashMap::new(),
    };
    grouper.group();
    let Grouper { groups, group_at, .. } = grouper;
    Comments {
        groups,
        scanned,
        group_at,
    }
}

fn collect(node: Node<'_>, source: &str, comments: &mut Vec<Comment>, items: &mut Vec<Scanned>) {
    let kind = node.kind();
    if kind == COMMENT {
        items.push(Scanned {
            offset: node.start_byte(),
            end: node.end_byte(),
            line: 0,
            kind: Kind::Comment(comments.len()),
            inserts_semicolon: false,
        });
        comments.push(Comment {
            slash: Pos::at(node.start_byte()),
            text: comment_text(source.get(node.byte_range()).unwrap_or_default()),
        });
        return;
    }
    if node.child_count() == 0 || STRING_KINDS.contains(&kind) {
        if !TERMINATOR_KINDS.contains(&kind) && node.end_byte() > node.start_byte() {
            items.push(Scanned {
                offset: node.start_byte(),
                end: node.end_byte(),
                line: 0,
                kind: token_kind(kind),
                inserts_semicolon: SEMICOLON_TRIGGERS.contains(&kind),
            });
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, comments, items);
    }
}

fn token_kind(kind: &str) -> Kind {
    if kind == SEMICOLON {
        Kind::Semicolon
    } else if CLOSING_KINDS.contains(&kind) {
        Kind::Closing
    } else {
        Kind::Other
    }
}

// Como go/scanner: a un `//` se le saca el `\r` final y a un `/* */` todos menos el de `*\r/`.
fn comment_text(text: &str) -> String {
    if text.starts_with(LINE_COMMENT_PREFIX) {
        return text.strip_suffix(CARRIAGE_RETURN).unwrap_or(text).to_owned();
    }
    if !text.contains(CARRIAGE_RETURN) {
        return text.to_owned();
    }
    let mut stripped = String::with_capacity(text.len());
    for (index, character) in text.char_indices() {
        let keeps = character != CARRIAGE_RETURN
            || text
                .get(index..)
                .is_some_and(|rest| rest.starts_with(CLOSING_SLASH));
        if keeps {
            stripped.push(character);
        }
    }
    stripped
}

// go/parser arma los grupos de comentarios mirando los tokens que los rodean, incluidos los punto y
// coma que inserta el scanner al final de una línea: se reconstruyen igual.
fn with_semicolons(
    items: &[Scanned],
    comments: &[Comment],
    source: &[u8],
    lines: &LineTable,
) -> Vec<Scanned> {
    let mut scanned = Vec::with_capacity(items.len());
    let mut inserts_semicolon = false;
    let mut previous_end = 0;
    for item in items {
        if inserts_semicolon && let Some(newline) = newline_between(source, previous_end, item.offset) {
            scanned.push(inserted_semicolon(lines, newline));
            inserts_semicolon = false;
        }
        let mut current = *item;
        current.line = lines.line(Pos::at(item.offset));
        scanned.push(current);
        if let Kind::Comment(index) = item.kind {
            let text = comments.get(index).map_or("", |comment| comment.text.as_str());
            if inserts_semicolon && !text.starts_with(LINE_COMMENT_PREFIX) && text.contains('\n') {
                scanned.push(inserted_semicolon(lines, item.offset));
                inserts_semicolon = false;
            }
        } else {
            inserts_semicolon = item.inserts_semicolon;
        }
        previous_end = item.end;
    }
    scanned
}

fn newline_between(source: &[u8], from: usize, to: usize) -> Option<usize> {
    source
        .get(from..to)?
        .iter()
        .position(|byte| *byte == NEWLINE)
        .map(|index| from.saturating_add(index))
}

fn inserted_semicolon(lines: &LineTable, offset: usize) -> Scanned {
    Scanned {
        offset,
        end: offset,
        line: lines.line(Pos::at(offset)),
        kind: Kind::InsertedSemicolon,
        inserts_semicolon: false,
    }
}

struct Grouper<'scan> {
    scanned: &'scan [Scanned],
    comments: &'scan [Comment],
    lines: &'scan LineTable,
    groups: Vec<CommentGroup>,
    group_at: HashMap<usize, usize>,
}

impl Grouper<'_> {
    fn group(&mut self) {
        let mut previous_line = 0;
        let mut index = 0;
        while let Some(item) = self.scanned.get(index) {
            if !matches!(item.kind, Kind::Comment(_)) {
                previous_line = item.line;
                index = index.saturating_add(1);
                continue;
            }
            if item.line == previous_line {
                index = self.take(index, 0);
            }
            while self.comment_at(index).is_some() {
                index = self.take(index, 1);
            }
        }
    }

    fn comment_at(&self, index: usize) -> Option<(i64, &Comment)> {
        let item = self.scanned.get(index)?;
        let Kind::Comment(position) = item.kind else {
            return None;
        };
        Some((item.line, self.comments.get(position)?))
    }

    fn take(&mut self, first: usize, slack: i64) -> usize {
        let mut list = Vec::new();
        let mut end_line = self.scanned.get(first).map_or(0, |item| item.line);
        let mut index = first;
        while let Some((line, comment)) = self.comment_at(index) {
            if line > end_line.saturating_add(slack) {
                break;
            }
            end_line = comment_end_line(comment, self.lines);
            list.push(comment.clone());
            index = index.saturating_add(1);
        }
        self.group_at.insert(first, self.groups.len());
        self.groups.push(CommentGroup { list });
        index
    }
}

fn comment_end_line(comment: &Comment, lines: &LineTable) -> i64 {
    let breaks = if comment.is_line_comment() {
        0
    } else {
        signed(comment.text.bytes().filter(|byte| *byte == NEWLINE).count())
    };
    lines.line(comment.slash).saturating_add(breaks)
}

impl Comments {
    // El comentario que go/parser le asigna a un campo o a una especificación que termina en
    // `end`: el que sigue en la misma línea, si después viene el punto y coma.
    pub fn line_comment(&self, end: usize) -> Option<usize> {
        let index = self.scanned.partition_point(|item| item.offset < end);
        let item = self.scanned.get(index)?;
        match item.kind {
            Kind::Semicolon => self.comment_after_semicolon(index),
            Kind::Comment(_) => {
                let group = *self.group_at.get(&index)?;
                let after = index.saturating_add(self.groups.get(group)?.list.len());
                match self.scanned.get(after).map(|next| next.kind) {
                    Some(Kind::InsertedSemicolon) => Some(group),
                    Some(Kind::Semicolon) => self.comment_after_semicolon(after),
                    Some(Kind::Closing | Kind::Other | Kind::Comment(_)) | None => None,
                }
            }
            Kind::InsertedSemicolon | Kind::Closing | Kind::Other => None,
        }
    }

    fn comment_after_semicolon(&self, semicolon: usize) -> Option<usize> {
        let line = self.scanned.get(semicolon)?.line;
        let first = semicolon.saturating_add(1);
        let item = self.scanned.get(first)?;
        if !matches!(item.kind, Kind::Comment(_)) || item.line != line {
            return None;
        }
        let group = *self.group_at.get(&first)?;
        let end_line = self
            .groups
            .get(group)?
            .list
            .last()
            .map_or(line, |last| comment_end_line_of(last, &self.scanned, first));
        let count = self.groups.get(group).map_or(0, |found| found.list.len());
        let next = self.scanned.get(first.saturating_add(count));
        let is_line_comment = next.is_none_or(|next| {
            next.line != end_line || matches!(next.kind, Kind::Semicolon | Kind::InsertedSemicolon)
        });
        is_line_comment.then_some(group)
    }
}

fn comment_end_line_of(last: &Comment, scanned: &[Scanned], first: usize) -> i64 {
    let start_line = scanned
        .iter()
        .skip(first)
        .find(|item| item.offset == last.slash.offset().unwrap_or_default())
        .map_or(0, |item| item.line);
    let breaks = if last.is_line_comment() {
        0
    } else {
        signed(last.text.bytes().filter(|byte| *byte == NEWLINE).count())
    };
    start_line.saturating_add(breaks)
}
