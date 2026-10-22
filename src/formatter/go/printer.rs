mod block_comment;
pub mod comments;
mod tabwriter;
mod trimmer;

use std::collections::HashMap;

use super::ast::CommentGroup;
use super::position::{INFINITY, LineTable, Pos, Position, signed};
use super::token::Token;
use super::{build_lines, nodes::NodeKey};
use comments::CommentCursor;

pub const MAX_NEWLINES: i64 = 2;
pub const ESCAPE: u8 = 0xff;
const WHITESPACE_CAPACITY: usize = 16;
const TAB: u8 = b'\t';
const VERTICAL_TAB: u8 = 0x0b;
const NEWLINE: u8 = b'\n';
const FORMFEED: u8 = 0x0c;
const SPACE: u8 = b' ';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    Ignore,
    Blank,
    Vtab,
    Newline,
    Formfeed,
    Indent,
    Unindent,
}

impl Whitespace {
    pub const fn byte(self) -> u8 {
        match self {
            Self::Vtab => VERTICAL_TAB,
            Self::Newline => NEWLINE,
            Self::Formfeed => FORMFEED,
            Self::Ignore | Self::Blank | Self::Indent | Self::Unindent => SPACE,
        }
    }

    pub const fn is_linebreak(self) -> bool {
        matches!(self, Self::Newline | Self::Formfeed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mode(u8);

impl Mode {
    pub const NO_EXTRA_BLANK: Self = Self(1);
    pub const NO_EXTRA_LINEBREAK: Self = Self(2);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Gofmt,
    Raw,
}

#[derive(Debug, Clone, Copy)]
struct Emitted {
    literal: bool,
    implies_semicolon: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Break {
    pub line: i64,
    pub min: i64,
    pub ws: Whitespace,
    pub section: bool,
}

#[derive(Debug)]
pub struct Printer<'context, 'sizes> {
    pub lines: &'context LineTable,
    pub style: Style,
    pub output: Vec<u8>,
    pub indent: i64,
    pub level: i64,
    pub mode: Mode,
    pub end_alignment: bool,
    pub implied_semi: bool,
    pub last_tok: Token,
    pub prev_open: Token,
    pub wsbuf: Vec<Whitespace>,
    pub go_build: Vec<usize>,
    pub plus_build: Vec<usize>,
    pub pos: Position,
    pub out: Position,
    pub last: Position,
    pub line_slot: Option<usize>,
    pub line_slots: Vec<i64>,
    pub comments: &'context [CommentGroup],
    pub cursor: CommentCursor,
    pub sizes: &'sizes mut HashMap<NodeKey, i64>,
}

impl<'context, 'sizes> Printer<'context, 'sizes> {
    pub fn new(
        lines: &'context LineTable,
        style: Style,
        comments: &'context [CommentGroup],
        sizes: &'sizes mut HashMap<NodeKey, i64>,
    ) -> Self {
        let mut printer = Self {
            lines,
            style,
            output: Vec::new(),
            indent: 0,
            level: 0,
            mode: Mode::default(),
            end_alignment: false,
            implied_semi: false,
            last_tok: Token::Illegal,
            prev_open: Token::Illegal,
            wsbuf: Vec::with_capacity(WHITESPACE_CAPACITY),
            go_build: Vec::new(),
            plus_build: Vec::new(),
            pos: Position::start(),
            out: Position::start(),
            last: Position::default(),
            line_slot: None,
            line_slots: Vec::new(),
            comments,
            cursor: CommentCursor::default(),
            sizes,
        };
        comments::next_comment(&mut printer);
        printer
    }

    pub fn pos_for(&self, pos: Pos) -> Position {
        self.lines.position(pos)
    }

    pub fn line_for(&self, pos: Pos) -> i64 {
        self.lines.line(pos)
    }

    pub fn set_pos(&mut self, pos: Pos) {
        if pos.is_valid() {
            self.pos = self.pos_for(pos);
        }
    }

    pub fn new_line_slot(&mut self) -> usize {
        self.line_slots.push(0);
        self.line_slots.len().saturating_sub(1)
    }

    pub const fn record_line(&mut self, slot: usize) {
        self.line_slot = Some(slot);
    }

    pub fn recorded_line(&self, slot: usize) -> i64 {
        self.line_slots.get(slot).copied().unwrap_or_default()
    }

    pub fn bump_recorded_line(&mut self, slot: usize) {
        if let Some(line) = self.line_slots.get_mut(slot) {
            *line = line.saturating_add(1);
        }
    }

    pub fn lines_from(&self, line: i64) -> i64 {
        self.out.line.saturating_sub(line)
    }

    fn update_prev_open(&mut self) {
        if self.last_tok == Token::Illegal {
            return;
        }
        self.prev_open = if matches!(self.last_tok, Token::Lparen | Token::Lbrack) {
            self.last_tok
        } else {
            Token::Illegal
        };
    }

    pub fn toggle(&mut self, mode: Mode) {
        self.update_prev_open();
        self.mode = Mode(self.mode.0 ^ mode.0);
    }

    pub fn ws(&mut self, ws: Whitespace) {
        self.update_prev_open();
        if ws == Whitespace::Ignore {
            return;
        }
        if self.wsbuf.len() == WHITESPACE_CAPACITY {
            comments::write_whitespace(self, WHITESPACE_CAPACITY);
        }
        self.wsbuf.push(ws);
        if ws.is_linebreak() {
            self.implied_semi = false;
        }
        self.last_tok = Token::Illegal;
    }

    pub fn token(&mut self, tok: Token) {
        self.update_prev_open();
        let text = tok.text();
        if text
            .bytes()
            .next()
            .is_some_and(|first| self.last_tok.combines_with(first))
        {
            self.wsbuf.clear();
            self.wsbuf.push(Whitespace::Blank);
        }
        self.last_tok = tok;
        let kind = Emitted {
            literal: false,
            implies_semicolon: tok.implies_semicolon(),
        };
        self.emit(text, kind);
    }

    pub fn ident(&mut self, name: &str) {
        self.update_prev_open();
        self.last_tok = Token::Ident;
        let kind = Emitted {
            literal: false,
            implies_semicolon: true,
        };
        self.emit(name, kind);
    }

    pub fn literal(&mut self, kind: Token, value: &str) {
        self.update_prev_open();
        self.last_tok = kind;
        let emitted = Emitted {
            literal: true,
            implies_semicolon: true,
        };
        self.emit(value, emitted);
    }

    fn emit(&mut self, data: &str, kind: Emitted) {
        let next = self.pos;
        let flushed = comments::flush(self, next, self.last_tok);
        let separated = !self.implied_semi && self.separate_lines(next, flushed);
        if let Some(slot) = self.line_slot.take()
            && let Some(line) = self.line_slots.get_mut(slot)
        {
            *line = self.out.line;
        }
        self.write_string(next, data, kind.literal);
        self.implied_semi = kind.implies_semicolon && !separated;
    }

    // Los saltos de línea del fuente que no meten un punto y coma se respetan, hasta uno en blanco.
    fn separate_lines(&mut self, next: Position, (wrote_newline, dropped_formfeed): (bool, bool)) -> bool {
        let mut count = limit_newlines(next.line.saturating_sub(self.pos.line));
        if wrote_newline && count == MAX_NEWLINES {
            count = MAX_NEWLINES.saturating_sub(1);
        }
        if count <= 0 {
            return false;
        }
        let byte = if dropped_formfeed { FORMFEED } else { NEWLINE };
        self.write_byte(byte, count);
        true
    }

    pub fn linebreak(&mut self, request: Break) -> i64 {
        let mut count = limit_newlines(request.line.saturating_sub(self.pos.line)).max(request.min);
        let mut breaks: i64 = 0;
        if count > 0 {
            self.ws(request.ws);
            if request.section {
                self.ws(Whitespace::Formfeed);
                count = count.saturating_sub(1);
                breaks = 2;
            }
            breaks = breaks.saturating_add(count);
            for _ in 0..count {
                self.ws(Whitespace::Newline);
            }
        }
        breaks
    }

    fn write_indent(&mut self) {
        let count = self.indent.max(0);
        for _ in 0..count {
            self.output.push(TAB);
        }
        self.pos.offset = self.pos.offset.saturating_add(count);
        self.pos.column = self.pos.column.saturating_add(count);
        self.out.column = self.out.column.saturating_add(count);
    }

    // Después de un literal con saltos de línea no se alinea más hasta el final de esa línea.
    const fn aligned_byte(&mut self, byte: u8) -> u8 {
        if !self.end_alignment {
            return byte;
        }
        if byte == TAB || byte == VERTICAL_TAB {
            return SPACE;
        }
        if byte == NEWLINE || byte == FORMFEED {
            self.end_alignment = false;
            return FORMFEED;
        }
        byte
    }

    pub fn write_byte(&mut self, byte: u8, count: i64) {
        let byte = self.aligned_byte(byte);
        if self.out.column == 1 {
            self.write_indent();
        }
        for _ in 0..count.max(0) {
            self.output.push(byte);
        }
        self.pos.offset = self.pos.offset.saturating_add(count);
        if byte == NEWLINE || byte == FORMFEED {
            self.pos.line = self.pos.line.saturating_add(count);
            self.out.line = self.out.line.saturating_add(count);
            self.pos.column = 1;
            self.out.column = 1;
            return;
        }
        self.pos.column = self.pos.column.saturating_add(count);
        self.out.column = self.out.column.saturating_add(count);
    }

    pub fn write_string(&mut self, pos: Position, text: &str, is_literal: bool) {
        if self.out.column == 1 {
            self.write_indent();
        }
        if pos.is_valid() {
            self.pos = pos;
        }
        if is_literal {
            self.output.push(ESCAPE);
        }
        self.output.extend_from_slice(text.as_bytes());
        let bytes = text.as_bytes();
        let breaks = bytes
            .iter()
            .filter(|byte| **byte == NEWLINE || **byte == FORMFEED)
            .count();
        let length = signed(text.len());
        self.pos.offset = self.pos.offset.saturating_add(length);
        if let Some(last_break) = bytes
            .iter()
            .rposition(|byte| *byte == NEWLINE || *byte == FORMFEED)
        {
            self.end_alignment = true;
            self.pos.line = self.pos.line.saturating_add(signed(breaks));
            self.out.line = self.out.line.saturating_add(signed(breaks));
            let column = length.saturating_sub(signed(last_break));
            self.pos.column = column;
            self.out.column = column;
        } else {
            self.pos.column = self.pos.column.saturating_add(length);
            self.out.column = self.out.column.saturating_add(length);
        }
        if is_literal {
            self.output.push(ESCAPE);
        }
        self.last = self.pos;
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.implied_semi = false;
        let end = Position {
            offset: INFINITY,
            line: INFINITY,
            column: 0,
        };
        comments::flush(&mut self, end, Token::Eof);
        build_lines::fix(&mut self);
        let output = std::mem::take(&mut self.output);
        match self.style {
            Style::Raw => trimmer::trim(&[output.as_slice()]),
            Style::Gofmt => tabwriter::align(&output),
        }
    }
}

pub fn limit_newlines(count: i64) -> i64 {
    count.min(MAX_NEWLINES)
}
