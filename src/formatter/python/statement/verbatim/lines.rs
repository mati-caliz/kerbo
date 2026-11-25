use crate::formatter::document::LineMode;
use crate::formatter::python::ast::Span;
use crate::formatter::python::context::Formatter;

const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const BACKSLASH: u8 = b'\\';
const COMMENT: u8 = b'#';
const QUOTES: [u8; 2] = *b"\"'";
const OPENING_BRACKETS: [u8; 3] = *b"([{";
const CLOSING_BRACKETS: [u8; 3] = *b")]}";
const TRIPLE_QUOTE_LENGTH: usize = 3;

#[derive(Debug, Clone, Copy)]
pub struct Indentation(usize);

const fn is_indent_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t')
}

impl Indentation {
    pub fn of_line_with(source: &str, offset: usize) -> Self {
        let before = source.get(..offset).unwrap_or_default();
        let line_start = before
            .rfind(['\n', '\r'])
            .map_or(0, |index| index.saturating_add(1));
        let line = source.get(line_start..offset).unwrap_or_default();
        Self(
            line.chars()
                .take_while(|character| is_indent_whitespace(*character))
                .count(),
        )
    }

    fn trim(self, source: &str, line: Span) -> Span {
        let text = source.get(line.start..line.end).unwrap_or_default();
        let trimmed = text
            .chars()
            .take(self.0)
            .take_while(|character| is_indent_whitespace(*character))
            .count();
        Span::new(line.start.saturating_add(trimmed), line.end)
    }
}

#[derive(Debug, Clone, Copy)]
struct LogicalLine {
    content: Span,
    has_trailing_newline: bool,
}

fn end_of_string(bytes: &[u8], start: usize) -> usize {
    let Some(quote) = bytes.get(start).copied() else {
        return start;
    };
    let is_triple =
        bytes.get(start..start.saturating_add(TRIPLE_QUOTE_LENGTH)) == Some(&[quote, quote, quote][..]);
    let mut index = start.saturating_add(if is_triple { TRIPLE_QUOTE_LENGTH } else { 1 });
    while let Some(byte) = bytes.get(index).copied() {
        if byte == BACKSLASH {
            index = index.saturating_add(2);
        } else if byte == quote {
            if !is_triple {
                return index.saturating_add(1);
            }
            if bytes.get(index..index.saturating_add(TRIPLE_QUOTE_LENGTH)) == Some(&[quote, quote, quote][..])
            {
                return index.saturating_add(TRIPLE_QUOTE_LENGTH);
            }
            index = index.saturating_add(1);
        } else if byte == LINE_FEED && !is_triple {
            return index;
        } else {
            index = index.saturating_add(1);
        }
    }
    bytes.len()
}

fn newline_length(bytes: &[u8], index: usize) -> usize {
    match (
        bytes.get(index).copied(),
        bytes.get(index.saturating_add(1)).copied(),
    ) {
        (Some(CARRIAGE_RETURN), Some(LINE_FEED)) => 2,
        (Some(CARRIAGE_RETURN | LINE_FEED), _) => 1,
        (Some(_) | None, _) => 0,
    }
}

// Las líneas lógicas de un tramo del fuente: una cadena de varias líneas o lo que está entre
// corchetes no las parte, igual que el lexer de ruff.
fn logical_lines(source: &str, range: Span) -> Vec<LogicalLine> {
    let bytes = source.as_bytes();
    let end = range.end.min(bytes.len());
    let mut lines = Vec::new();
    let mut line_start = range.start;
    let mut depth = 0_usize;
    let mut index = range.start;
    while index < end {
        let Some(byte) = bytes.get(index).copied() else {
            break;
        };
        let newline = newline_length(bytes, index);
        if newline > 0 {
            if depth == 0 {
                let full_end = index.saturating_add(newline);
                lines.push(LogicalLine {
                    content: Span::new(line_start, index),
                    has_trailing_newline: true,
                });
                line_start = full_end;
            }
            index = index.saturating_add(newline);
        } else if byte == COMMENT {
            while index < end && newline_length(bytes, index) == 0 {
                index = index.saturating_add(1);
            }
        } else if QUOTES.contains(&byte) {
            index = end_of_string(bytes, index);
        } else if byte == BACKSLASH {
            let after = index.saturating_add(1);
            index = after.saturating_add(newline_length(bytes, after).max(1));
        } else {
            if OPENING_BRACKETS.contains(&byte) {
                depth = depth.saturating_add(1);
            } else if CLOSING_BRACKETS.contains(&byte) {
                depth = depth.saturating_sub(1);
            }
            index = index.saturating_add(1);
        }
    }
    if line_start < end {
        lines.push(LogicalLine {
            content: Span::new(line_start, end),
            has_trailing_newline: false,
        });
    }
    lines
}

fn normalized_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn write_verbatim_text(f: &mut Formatter<'_>, range: Span) {
    f.verbatim_ranges.push(range);
    let text = f.text_of(range);
    if text.contains('\r') {
        f.text(&normalized_newlines(text));
    } else {
        f.source_text(range);
    }
}

const fn blank_line_mode(is_first: bool) -> LineMode {
    if is_first { LineMode::Hard } else { LineMode::Empty }
}

// Cada línea lógica del tramo tal como está en el fuente, sangrada como las sentencias que sí se
// formatean.
pub fn write_verbatim_range(f: &mut Formatter<'_>, range: Span, indentation: Indentation) {
    let mut is_first = true;
    for line in logical_lines(f.source, range) {
        let trimmed = indentation.trim(f.source, line.content);
        let has_content = trimmed.start < trimmed.end;
        if has_content {
            write_verbatim_text(f, trimmed);
        }
        if line.has_trailing_newline {
            f.line(if has_content {
                LineMode::Hard
            } else {
                blank_line_mode(is_first)
            });
        }
        is_first = false;
    }
}
