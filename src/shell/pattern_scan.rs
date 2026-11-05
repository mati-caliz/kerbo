const ESCAPE: char = '\\';
const SINGLE_QUOTE: char = '\'';
const DOUBLE_QUOTE: char = '"';
const DOLLAR: char = '$';
const BACKTICK: char = '`';
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const SPECIAL_PARAMETERS: &[char] = &['*', '@', '#', '?', '-', '$', '!'];
const UNDERSCORE: char = '_';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Plain,
    SingleQuoted,
    DoubleQuoted,
    Expansion,
    Substitution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub start: usize,
    pub end: usize,
}

// Parte el texto de una hoja `regex` de tree-sitter, que junta en un solo nodo el patrón de `${x%…}`
// o el lado derecho de `=` en `[[ ]]`, con comillas y expansiones adentro.
pub fn segments(pattern: &str) -> Vec<Segment> {
    let mut found = Vec::new();
    let mut plain_start = 0;
    let mut index = 0;
    while let Some(rest) = pattern.get(index..).filter(|rest| !rest.is_empty()) {
        if let Some(escaped_length) = escaped_length(rest) {
            index = index.saturating_add(escaped_length);
            continue;
        }
        let Some((kind, length)) = special_segment(rest) else {
            index = index.saturating_add(rest.chars().next().map_or(1, char::len_utf8));
            continue;
        };
        if plain_start < index {
            found.push(Segment {
                kind: SegmentKind::Plain,
                start: plain_start,
                end: index,
            });
        }
        let end = index.saturating_add(length);
        found.push(Segment {
            kind,
            start: index,
            end,
        });
        index = end;
        plain_start = end;
    }
    if plain_start < pattern.len() {
        found.push(Segment {
            kind: SegmentKind::Plain,
            start: plain_start,
            end: pattern.len(),
        });
    }
    found
}

fn escaped_length(rest: &str) -> Option<usize> {
    let mut characters = rest.chars();
    if characters.next() != Some(ESCAPE) {
        return None;
    }
    let escaped = characters.next().map_or(0, char::len_utf8);
    Some(ESCAPE.len_utf8().saturating_add(escaped))
}

fn special_segment(rest: &str) -> Option<(SegmentKind, usize)> {
    let mut characters = rest.chars();
    let first = characters.next()?;
    match first {
        SINGLE_QUOTE => Some((SegmentKind::SingleQuoted, closing(rest, SINGLE_QUOTE)?)),
        DOUBLE_QUOTE => Some((SegmentKind::DoubleQuoted, closing_double_quote(rest)?)),
        BACKTICK => Some((SegmentKind::Substitution, closing(rest, BACKTICK)?)),
        DOLLAR => dollar_segment(rest, characters.next()?),
        _ => None,
    }
}

fn dollar_segment(rest: &str, next: char) -> Option<(SegmentKind, usize)> {
    let after_dollar = rest.get(DOLLAR.len_utf8()..)?;
    match next {
        OPEN_PARENTHESIS => Some((
            SegmentKind::Substitution,
            DOLLAR
                .len_utf8()
                .saturating_add(nested(after_dollar, OPEN_PARENTHESIS, CLOSE_PARENTHESIS)?),
        )),
        OPEN_BRACE => Some((
            SegmentKind::Expansion,
            DOLLAR
                .len_utf8()
                .saturating_add(nested(after_dollar, OPEN_BRACE, CLOSE_BRACE)?),
        )),
        _ if next.is_ascii_digit() || SPECIAL_PARAMETERS.contains(&next) => Some((
            SegmentKind::Expansion,
            DOLLAR.len_utf8().saturating_add(next.len_utf8()),
        )),
        _ if next.is_ascii_alphabetic() || next == UNDERSCORE => {
            let name_length = after_dollar
                .find(|character: char| !character.is_ascii_alphanumeric() && character != UNDERSCORE)
                .unwrap_or(after_dollar.len());
            Some((
                SegmentKind::Expansion,
                DOLLAR.len_utf8().saturating_add(name_length),
            ))
        }
        _ => None,
    }
}

fn closing(rest: &str, quote: char) -> Option<usize> {
    let after_open = rest.get(quote.len_utf8()..)?;
    let close = after_open.find(quote)?;
    Some(
        quote
            .len_utf8()
            .saturating_add(close)
            .saturating_add(quote.len_utf8()),
    )
}

fn closing_double_quote(rest: &str) -> Option<usize> {
    let mut characters = rest.char_indices().skip(1);
    while let Some((index, character)) = characters.next() {
        if character == ESCAPE {
            characters.next();
        } else if character == DOUBLE_QUOTE {
            return Some(index.saturating_add(DOUBLE_QUOTE.len_utf8()));
        }
    }
    None
}

fn nested(rest: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, character) in rest.char_indices() {
        if character == open {
            depth = depth.saturating_add(1);
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index.saturating_add(close.len_utf8()));
            }
        }
    }
    None
}
