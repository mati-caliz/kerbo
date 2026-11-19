use super::token_kind::{OPERATORS, STRING_PREFIXES, TokenKind, keyword_or_name};
use super::{is_line_break, is_python_whitespace};
use crate::formatter::python::ast::Span;

const COMMENT_START: char = '#';
const CARRIAGE_RETURN: char = '\r';
const WINDOWS_LINE_ENDING: &str = "\r\n";
const QUOTES: [char; 2] = ['"', '\''];
const BACKWARD_OPERATOR_ENDS: [char; 8] = ['=', '*', '/', '@', '!', '<', '>', '.'];
const OPERATOR_CHARACTERS: [char; 16] = [
    ':', '~', '%', '|', '&', '^', '+', '-', '=', '*', '/', '@', '!', '<', '>', '.',
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continuation(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn length_while(text: &str, keeps: impl Fn(char) -> bool) -> usize {
    text.find(|character| !keeps(character)).unwrap_or(text.len())
}

fn length_back_while(text: &str, keeps: impl Fn(char) -> bool) -> usize {
    text.len().saturating_sub(text.trim_end_matches(keeps).len())
}

// Port de `SimpleTokenizer` de ruff_python_trivia: separa lo que hay entre dos nodos (paréntesis,
// comas, comentarios, palabras clave) sin conocer la sintaxis. Lo que no reconoce, como el
// comienzo de una cadena, lo corta: de ahí en más devuelve un solo `Bogus`.
#[derive(Debug, Clone)]
pub struct Tokenizer<'source> {
    source: &'source str,
    offset: usize,
    end: usize,
    bogus: bool,
}

impl<'source> Tokenizer<'source> {
    pub const fn new(source: &'source str, span: Span) -> Self {
        Self {
            source,
            offset: span.start,
            end: span.end,
            bogus: false,
        }
    }

    pub const fn starts_at(offset: usize, source: &'source str) -> Self {
        Self::new(source, Span::new(offset, source.len()))
    }

    pub fn skip_trivia(self) -> impl Iterator<Item = Token> + 'source {
        self.filter(|token| !token.kind.is_trivia())
    }

    fn rest(&self) -> &'source str {
        self.source.get(self.offset..self.end).unwrap_or_default()
    }

    fn word(&mut self, rest: &str) -> (TokenKind, usize) {
        let length = length_while(rest, is_identifier_continuation);
        let text = rest.get(..length).unwrap_or_default();
        let kind = keyword_or_name(text);
        let opens_a_string = rest.get(length..).is_some_and(|after| after.starts_with(QUOTES));
        if kind == TokenKind::Name && opens_a_string && STRING_PREFIXES.contains(&text) {
            self.bogus = true;
            return (TokenKind::Other, length);
        }
        (kind, length)
    }

    fn operator(&mut self, rest: &str, first: char) -> (TokenKind, usize) {
        let operator = OPERATORS.iter().find(|(text, _)| rest.starts_with(text));
        if let Some((text, kind)) = operator {
            return (*kind, text.len());
        }
        self.bogus = true;
        (TokenKind::Other, first.len_utf8())
    }

    fn lex(&mut self, rest: &str, first: char) -> (TokenKind, usize) {
        if is_identifier_start(first) {
            self.word(rest)
        } else if is_python_whitespace(first) {
            (TokenKind::Whitespace, length_while(rest, is_python_whitespace))
        } else if rest.starts_with(WINDOWS_LINE_ENDING) {
            (TokenKind::Newline, WINDOWS_LINE_ENDING.len())
        } else if is_line_break(first) {
            (TokenKind::Newline, first.len_utf8())
        } else if first == COMMENT_START {
            (
                TokenKind::Comment,
                length_while(rest, |character| !is_line_break(character)),
            )
        } else {
            self.operator(rest, first)
        }
    }
}

impl Iterator for Tokenizer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let rest = self.rest();
        let first = rest.chars().next()?;
        let (kind, length) = if self.bogus {
            (TokenKind::Bogus, self.source.len().saturating_sub(self.offset))
        } else {
            self.lex(rest, first)
        };
        let start = self.offset;
        self.offset = start.saturating_add(length);
        if kind == TokenKind::Bogus {
            self.end = self.offset;
        }
        Some(Token {
            kind,
            span: Span::new(start, self.offset),
        })
    }
}

// Port de `BackwardsTokenizer`: lo mismo desde el final hacia atrás. Un comentario no se reconoce
// leyendo al revés, así que recibe los tramos de los comentarios del archivo.
#[derive(Debug, Clone)]
pub struct BackwardsTokenizer<'source> {
    source: &'source str,
    start: usize,
    back_offset: usize,
    comments: &'source [Span],
    bogus: bool,
}

impl<'source> BackwardsTokenizer<'source> {
    pub fn up_to(offset: usize, source: &'source str, comments: &'source [Span]) -> Self {
        let before = comments.partition_point(|comment| comment.start <= offset);
        Self {
            source,
            start: 0,
            back_offset: offset,
            comments: comments.get(..before).unwrap_or_default(),
            bogus: false,
        }
    }

    pub fn skip_trivia(self) -> impl Iterator<Item = Token> + 'source {
        self.filter(|token| !token.kind.is_trivia())
    }

    fn rest(&self) -> &'source str {
        self.source.get(self.start..self.back_offset).unwrap_or_default()
    }

    fn comment(&mut self) -> Option<Token> {
        let (comment, before) = self.comments.split_last()?;
        if comment.start > self.back_offset || self.back_offset > comment.end {
            return None;
        }
        self.comments = before;
        self.back_offset = comment.start;
        Some(Token {
            kind: TokenKind::Comment,
            span: *comment,
        })
    }

    fn word(&mut self, rest: &str, last: char) -> (TokenKind, usize) {
        let length = length_back_while(rest, is_identifier_continuation);
        let text = rest.get(rest.len().saturating_sub(length)..).unwrap_or_default();
        if text.starts_with(is_identifier_start) {
            return (keyword_or_name(text), length);
        }
        self.bogus = true;
        (TokenKind::Other, last.len_utf8())
    }

    // Un operador puede ser el final de otro más largo (`=` de `**=`): se toma toda la tira de
    // caracteres de operador, se la lee hacia adelante y vale su último token.
    fn operator(&mut self, rest: &str, last: char) -> (TokenKind, usize) {
        if !BACKWARD_OPERATOR_ENDS.contains(&last) {
            let single = OPERATORS
                .iter()
                .find(|(text, _)| text.len() == last.len_utf8() && text.starts_with(last));
            if let Some((_, kind)) = single {
                return (*kind, last.len_utf8());
            }
            self.bogus = true;
            return (TokenKind::Other, last.len_utf8());
        }
        let run = length_back_while(rest, |character| OPERATOR_CHARACTERS.contains(&character));
        let run_span = Span::new(self.back_offset.saturating_sub(run), self.back_offset);
        if let Some(token) = Tokenizer::new(self.source, run_span).last() {
            return (
                token.kind,
                token.span.end.saturating_sub(token.span.start).min(run),
            );
        }
        self.bogus = true;
        (TokenKind::Other, last.len_utf8())
    }

    fn lex(&mut self, rest: &str, last: char) -> (TokenKind, usize) {
        if is_python_whitespace(last) {
            (
                TokenKind::Whitespace,
                length_back_while(rest, is_python_whitespace),
            )
        } else if rest.ends_with(WINDOWS_LINE_ENDING) {
            (TokenKind::Newline, WINDOWS_LINE_ENDING.len())
        } else if is_line_break(last) || last == CARRIAGE_RETURN {
            (TokenKind::Newline, last.len_utf8())
        } else if is_identifier_continuation(last) {
            self.word(rest, last)
        } else {
            self.operator(rest, last)
        }
    }
}

impl Iterator for BackwardsTokenizer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let rest = self.rest();
        let last = rest.chars().next_back()?;
        if self.bogus {
            let span = Span::new(self.start, self.back_offset);
            self.back_offset = self.start;
            return Some(Token {
                kind: TokenKind::Bogus,
                span,
            });
        }
        if let Some(comment) = self.comment() {
            return Some(comment);
        }
        let (kind, length) = self.lex(rest, last);
        let end = self.back_offset;
        self.back_offset = end.saturating_sub(length);
        Some(Token {
            kind,
            span: Span::new(self.back_offset, end),
        })
    }
}
