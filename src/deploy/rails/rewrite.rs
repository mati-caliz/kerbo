use std::ops::Range;

use tree_sitter::{Node, Parser, Tree};

use super::arguments::{name, raw};
use super::{locals, loops};
use crate::language::Language;

const INTEGER: &str = "integer";
const SYMBOL_KINDS: &[&str] = &["simple_symbol", "delimited_symbol", "bare_symbol"];
const UNSAFE_CHARACTERS: &[char] = &['"', '\\', '#', '\n'];
const NEWLINE: u8 = b'\n';

pub struct Expansion {
    pub source: String,
    pub rows: Vec<usize>,
}

impl Expansion {
    #[must_use]
    pub fn then(self, next: Self) -> Self {
        let rows = next
            .rows
            .iter()
            .map(|row| self.rows.get(*row).copied().unwrap_or(*row))
            .collect();
        Self {
            source: next.source,
            rows,
        }
    }
}

pub struct Value {
    pub literal: String,
    pub text: String,
}

impl Value {
    pub fn rendered(&self, interpolated: bool) -> &str {
        if interpolated { &self.text } else { &self.literal }
    }
}

pub fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&Language::Rails.grammar()?).ok()?;
    parser.parse(source, None)
}

pub fn rewritten(root: Node<'_>, source: &[u8]) -> Option<Expansion> {
    let Some(unrolled) = loops::expand(root, source) else {
        return locals::inline(root, source);
    };
    let inlined =
        parse(&unrolled.source).and_then(|tree| locals::inline(tree.root_node(), unrolled.source.as_bytes()));
    Some(match inlined {
        Some(inlined) => unrolled.then(inlined),
        None => unrolled,
    })
}

pub fn value(node: Node<'_>, source: &[u8]) -> Option<Value> {
    let text = if node.kind() == INTEGER {
        raw(node, source)
    } else {
        name(node, source)?
    };
    if text.contains(UNSAFE_CHARACTERS) {
        return None;
    }
    let literal = match node.kind() {
        INTEGER => text.clone(),
        kind if SYMBOL_KINDS.contains(&kind) => format!(":\"{text}\""),
        _ => format!("\"{text}\""),
    };
    Some(Value { literal, text })
}

pub struct Emitter<'source> {
    source: &'source [u8],
    line_starts: Vec<usize>,
    text: String,
    rows: Vec<usize>,
}

impl<'source> Emitter<'source> {
    pub fn new(source: &'source [u8]) -> Self {
        let line_starts = std::iter::once(0)
            .chain(
                source
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == NEWLINE)
                    .map(|(offset, _)| offset + 1),
            )
            .collect();
        Self {
            source,
            line_starts,
            text: String::new(),
            rows: vec![0],
        }
    }

    pub fn row_of(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    pub fn original(&mut self, range: Range<usize>) {
        let mut row = self.row_of(range.start);
        let piece = String::from_utf8_lossy(self.source.get(range).unwrap_or_default()).into_owned();
        for _ in piece.matches(char::from(NEWLINE)) {
            row += 1;
            self.rows.push(row);
        }
        self.text.push_str(&piece);
    }

    pub fn inserted(&mut self, text: &str, row: usize) {
        for _ in text.matches(char::from(NEWLINE)) {
            self.rows.push(row);
        }
        self.text.push_str(text);
    }

    pub fn finish(self) -> Expansion {
        Expansion {
            source: self.text,
            rows: self.rows,
        }
    }
}
