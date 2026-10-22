use super::super::ast::Comment;
use super::super::build_lines::{is_go_build, is_plus_build};
use super::super::position::{INFINITY, Position, signed};
use super::super::token::Token;
use super::block_comment::strip_common_prefix;
use super::{FORMFEED, Mode, Printer, Whitespace, limit_newlines};

const LINE_DIRECTIVE: &str = "//line ";
const INDENT_FOR_FIRST_COLUMN: &str = "   ";

#[derive(Debug, Clone, Copy)]
pub struct CommentCursor {
    pub next: usize,
    pub current: Option<usize>,
    pub offset: i64,
    pub has_newline: bool,
}

impl Default for CommentCursor {
    fn default() -> Self {
        Self {
            next: 0,
            current: None,
            offset: INFINITY,
            has_newline: false,
        }
    }
}

pub fn next_comment(printer: &mut Printer<'_, '_>) {
    while let Some(group) = printer.comments.get(printer.cursor.next) {
        let index = printer.cursor.next;
        printer.cursor.next = index.saturating_add(1);
        if let Some(first) = group.list.first() {
            printer.cursor.current = Some(index);
            printer.cursor.offset = printer.pos_for(first.slash).offset;
            printer.cursor.has_newline = have_newline(printer, &group.list);
            return;
        }
    }
    printer.cursor.offset = INFINITY;
}

fn have_newline(printer: &Printer<'_, '_>, list: &[Comment]) -> bool {
    let line = list.first().map_or(0, |first| printer.line_for(first.slash));
    list.iter().enumerate().any(|(index, comment)| {
        (index > 0 && printer.line_for(comment.slash) != line)
            || (comment.text.len() >= 2 && (comment.is_line_comment() || comment.text.contains('\n')))
    })
}

pub fn comment_before(printer: &Printer<'_, '_>, next: Position) -> bool {
    printer.cursor.offset < next.offset && (!printer.implied_semi || !printer.cursor.has_newline)
}

pub fn comment_size_before(printer: &mut Printer<'_, '_>, next: Position) -> i64 {
    let saved = printer.cursor;
    let mut size: i64 = 0;
    while comment_before(printer, next) {
        let comments = printer.comments;
        if let Some(group) = printer.cursor.current.and_then(|index| comments.get(index)) {
            let length: usize = group.list.iter().map(|comment| comment.text.len()).sum();
            size = size.saturating_add(signed(length));
        }
        next_comment(printer);
    }
    printer.cursor = saved;
    size
}

pub fn flush(printer: &mut Printer<'_, '_>, next: Position, tok: Token) -> (bool, bool) {
    if comment_before(printer, next) {
        return intersperse_comments(printer, next, tok);
    }
    let pending = printer.wsbuf.len();
    write_whitespace(printer, pending);
    (false, false)
}

#[derive(Debug, Clone, Copy)]
struct Spot<'comment> {
    pos: Position,
    next: Position,
    prev: Option<&'comment Comment>,
    tok: Token,
}

fn intersperse_comments(printer: &mut Printer<'_, '_>, next: Position, tok: Token) -> (bool, bool) {
    let groups = printer.comments;
    let mut last: Option<Comment> = None;
    while comment_before(printer, next) {
        let Some(group) = printer.cursor.current.and_then(|index| groups.get(index)) else {
            break;
        };
        let is_doc = tok != Token::Ident
            && printer.last_tok != Token::Import
            && printer.pos_for(group.pos()).column == 1
            && printer.pos_for(group.end().shifted(1)) == next;
        let list = if is_doc {
            format_doc_comment(&group.list)
        } else {
            group.list.clone()
        };
        for current in list {
            let spot = Spot {
                pos: printer.pos_for(current.slash),
                next,
                prev: last.as_ref(),
                tok,
            };
            write_comment_prefix(printer, &spot);
            write_comment(printer, &current);
            last = Some(current);
        }
        if is_doc && let Some(original) = group.list.last() {
            printer.pos = printer.pos_for(original.end());
            printer.last = printer.pos;
            last = Some(original.clone());
        }
        next_comment(printer);
    }
    let Some(last) = last else {
        return (false, false);
    };
    let needs_linebreak = separate_after_comment(printer, &last, next, tok);
    write_comment_suffix(printer, needs_linebreak)
}

fn separate_after_comment(printer: &mut Printer<'_, '_>, last: &Comment, next: Position, tok: Token) -> bool {
    let mut needs_linebreak = false;
    if !printer.mode.has(Mode::NO_EXTRA_BLANK)
        && !last.is_line_comment()
        && printer.line_for(last.slash) == next.line
        && tok != Token::Comma
        && (tok != Token::Rparen || printer.prev_open == Token::Lparen)
        && (tok != Token::Rbrack || printer.prev_open == Token::Lbrack)
    {
        if contains_linebreak(printer) && !printer.mode.has(Mode::NO_EXTRA_LINEBREAK) && printer.level == 0 {
            needs_linebreak = true;
        } else {
            printer.write_byte(b' ', 1);
        }
    }
    if last.is_line_comment()
        || tok == Token::Eof
        || (tok == Token::Rbrace && !printer.mode.has(Mode::NO_EXTRA_LINEBREAK))
    {
        needs_linebreak = true;
    }
    needs_linebreak
}

// Con el texto de un comentario de documentación ya canónico, `formatDocComment` de go/printer
// devuelve las mismas líneas pero todas con la posición de la primera.
fn format_doc_comment(list: &[Comment]) -> Vec<Comment> {
    let slash = list.first().map(|first| first.slash).unwrap_or_default();
    list.iter()
        .map(|comment| Comment {
            slash,
            text: comment.text.clone(),
        })
        .collect()
}

fn write_comment_prefix(printer: &mut Printer<'_, '_>, spot: &Spot<'_>) {
    if printer.output.is_empty() {
        return;
    }
    if spot.pos.line == printer.last.line && spot.prev.is_none_or(|previous| !previous.is_line_comment()) {
        prefix_on_same_line(printer, spot.pos, spot.next, spot.prev.is_none());
    } else {
        prefix_on_new_line(printer, spot);
    }
}

fn prefix_on_same_line(printer: &mut Printer<'_, '_>, pos: Position, next: Position, first_of_group: bool) {
    let has_separator = first_of_group && consume_blanks_before_comment(printer);
    if !has_separator {
        let separator = if pos.line == next.line { b' ' } else { b'\t' };
        printer.write_byte(separator, 1);
    }
}

fn consume_blanks_before_comment(printer: &mut Printer<'_, '_>) -> bool {
    let mut has_separator = false;
    let mut stop = 0;
    for index in 0..printer.wsbuf.len() {
        match printer.wsbuf.get(index).copied() {
            Some(Whitespace::Blank) => set_ignored(printer, index),
            Some(Whitespace::Vtab) => has_separator = true,
            Some(Whitespace::Indent) => {}
            Some(Whitespace::Ignore | Whitespace::Newline | Whitespace::Formfeed | Whitespace::Unindent) | None => {
                stop = index;
                break;
            }
        }
    }
    write_whitespace(printer, stop);
    has_separator
}

fn prefix_on_new_line(printer: &mut Printer<'_, '_>, spot: &Spot<'_>) {
    let dropped_linebreak = consume_whitespace_before_line(printer, spot);
    let mut count = if spot.pos.is_valid() && printer.last.is_valid() {
        spot.pos.line.saturating_sub(printer.last.line).max(0)
    } else {
        0
    };
    if printer.indent == 0 && dropped_linebreak {
        count = count.saturating_add(1);
    }
    if count == 0 && spot.prev.is_some_and(Comment::is_line_comment) {
        count = 1;
    }
    if count > 0 {
        printer.write_byte(FORMFEED, limit_newlines(count));
    }
}

fn consume_whitespace_before_line(printer: &mut Printer<'_, '_>, spot: &Spot<'_>) -> bool {
    let mut dropped_linebreak = false;
    let mut stop = 0;
    for index in 0..printer.wsbuf.len() {
        match printer.wsbuf.get(index).copied() {
            Some(Whitespace::Blank | Whitespace::Vtab) => {
                set_ignored(printer, index);
                continue;
            }
            Some(Whitespace::Indent) => continue,
            Some(Whitespace::Unindent) => {
                if keeps_unindent(printer, index, spot) {
                    continue;
                }
            }
            Some(Whitespace::Newline | Whitespace::Formfeed) => {
                set_ignored(printer, index);
                dropped_linebreak = spot.prev.is_none();
            }
            Some(Whitespace::Ignore) | None => {}
        }
        stop = index;
        break;
    }
    write_whitespace(printer, stop);
    dropped_linebreak
}

// Un `unindent` que no es el último, o uno de un comentario alineado con lo que sigue, es del
// bloque anterior (una lista de varias líneas) y se aplica antes del comentario.
fn keeps_unindent(printer: &Printer<'_, '_>, index: usize, spot: &Spot<'_>) -> bool {
    let followed_by_unindent = printer.wsbuf.get(index.saturating_add(1)) == Some(&Whitespace::Unindent);
    followed_by_unindent || (spot.tok != Token::Rbrace && spot.pos.column == spot.next.column)
}

fn set_ignored(printer: &mut Printer<'_, '_>, index: usize) {
    if let Some(slot) = printer.wsbuf.get_mut(index) {
        *slot = Whitespace::Ignore;
    }
}

fn write_comment(printer: &mut Printer<'_, '_>, comment: &Comment) {
    let text = comment.text.as_str();
    let mut pos = printer.pos_for(comment.slash);
    let saved_indent = printer.indent;
    if text.starts_with(LINE_DIRECTIVE) && (!pos.is_valid() || pos.column == 1) {
        printer.indent = 0;
    }
    if comment.is_line_comment() {
        if is_go_build(text) {
            printer.go_build.push(printer.output.len());
        } else if is_plus_build(text) {
            printer.plus_build.push(printer.output.len());
        }
        printer.write_string(pos, text.trim_end(), true);
        printer.indent = saved_indent;
        return;
    }
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    if pos.is_valid() && pos.column == 1 && printer.indent > 0 {
        for line in lines.iter_mut().skip(1) {
            line.insert_str(0, INDENT_FOR_FIRST_COLUMN);
        }
    }
    strip_common_prefix(&mut lines);
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            printer.write_byte(FORMFEED, 1);
            pos = printer.pos;
        }
        if !line.is_empty() {
            printer.write_string(pos, line.trim_end(), true);
        }
    }
    printer.indent = saved_indent;
}

fn write_comment_suffix(printer: &mut Printer<'_, '_>, needs_linebreak: bool) -> (bool, bool) {
    let mut needs_linebreak = needs_linebreak;
    let mut wrote_newline = false;
    let mut dropped_formfeed = false;
    for index in 0..printer.wsbuf.len() {
        match printer.wsbuf.get(index).copied() {
            Some(Whitespace::Blank | Whitespace::Vtab) => set_ignored(printer, index),
            Some(whitespace @ (Whitespace::Newline | Whitespace::Formfeed)) => {
                if needs_linebreak {
                    needs_linebreak = false;
                    wrote_newline = true;
                } else {
                    dropped_formfeed |= whitespace == Whitespace::Formfeed;
                    set_ignored(printer, index);
                }
            }
            Some(Whitespace::Indent | Whitespace::Unindent | Whitespace::Ignore) | None => {}
        }
    }
    let pending = printer.wsbuf.len();
    write_whitespace(printer, pending);
    if needs_linebreak {
        printer.write_byte(b'\n', 1);
        wrote_newline = true;
    }
    (wrote_newline, dropped_formfeed)
}

fn contains_linebreak(printer: &Printer<'_, '_>) -> bool {
    printer.wsbuf.iter().any(|whitespace| whitespace.is_linebreak())
}

pub fn write_whitespace(printer: &mut Printer<'_, '_>, count: usize) {
    let mut index = 0;
    while index < count {
        let Some(current) = printer.wsbuf.get(index).copied() else {
            break;
        };
        match current {
            Whitespace::Ignore => {}
            Whitespace::Indent => printer.indent = printer.indent.saturating_add(1),
            Whitespace::Unindent => printer.indent = printer.indent.saturating_sub(1).max(0),
            Whitespace::Newline | Whitespace::Formfeed => {
                if swaps_with_unindent(printer, index, count) {
                    continue;
                }
                printer.write_byte(current.byte(), 1);
            }
            Whitespace::Blank | Whitespace::Vtab => printer.write_byte(current.byte(), 1),
        }
        index = index.saturating_add(1);
    }
    let remaining = count.min(printer.wsbuf.len());
    printer.wsbuf.drain(..remaining);
}

// Un salto de línea seguido de un `unindent` que lo corrige se cambia de lugar con él (así quedan
// bien las etiquetas) y pasa a ser un salto de página, que cierra la sección de alineación.
fn swaps_with_unindent(printer: &mut Printer<'_, '_>, index: usize, count: usize) -> bool {
    let next = index.saturating_add(1);
    if next >= count || printer.wsbuf.get(next) != Some(&Whitespace::Unindent) {
        return false;
    }
    printer.wsbuf.swap(index, next);
    if let Some(slot) = printer.wsbuf.get_mut(next) {
        *slot = Whitespace::Formfeed;
    }
    true
}
