use super::context::Position;
use super::word_pieces::{Piece, PieceKind};

const ESCAPE: char = '\\';
const NEWLINE: char = '\n';
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const OPEN_BRACKET: char = '[';
const CLOSE_BRACKET: char = ']';
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const BRACE_LIST: char = ',';
const BRACE_RANGE: &str = "..";
const SIMPLE_GLOBS: &[char] = &['*', '?'];
const LONE_EXTGLOB_STARTS: &[char] = &['+', '@', '!'];
const EXTGLOB_STARTS: &[char] = &['?', '*', '+', '@', '!'];
const CLASS_NEGATIONS: &[char] = &['!', '^'];

// `readNormalWord` de ShellCheck sobre el texto de una hoja sin comillas.
pub fn scan_unquoted(raw: &str, start: Position) -> Vec<Piece> {
    let mut found = Vec::new();
    let mut pending = String::new();
    let mut pending_start = 0;
    let mut index = 0;
    while let Some(rest) = raw.get(index..).filter(|rest| !rest.is_empty()) {
        let Some(special) = special_piece(rest) else {
            let (consumed, value) = literal_character(rest);
            if pending.is_empty() {
                pending_start = index;
            }
            pending.push_str(&value);
            index = index.saturating_add(consumed);
            continue;
        };
        if !pending.is_empty() {
            let literal_text = std::mem::take(&mut pending);
            found.push(Piece::new(
                PieceKind::Literal,
                literal_text,
                start.shifted(pending_start),
            ));
        }
        let (kind, consumed) = special;
        let matched = rest.get(..consumed).unwrap_or_default().to_owned();
        found.push(Piece::new(kind, matched, start.shifted(index)));
        index = index.saturating_add(consumed);
    }
    if !pending.is_empty() {
        found.push(Piece::new(
            PieceKind::Literal,
            pending,
            start.shifted(pending_start),
        ));
    }
    found
}

fn literal_character(rest: &str) -> (usize, String) {
    let mut characters = rest.chars();
    let Some(first) = characters.next() else {
        return (0, String::new());
    };
    if first != ESCAPE {
        return (first.len_utf8(), first.to_string());
    }
    match characters.next() {
        Some(NEWLINE) => (first.len_utf8().saturating_add(NEWLINE.len_utf8()), String::new()),
        Some(escaped) => (
            first.len_utf8().saturating_add(escaped.len_utf8()),
            escaped.to_string(),
        ),
        None => (first.len_utf8(), String::new()),
    }
}

fn special_piece(rest: &str) -> Option<(PieceKind, usize)> {
    let mut characters = rest.chars();
    let first = characters.next()?;
    let second = characters.next();
    if EXTGLOB_STARTS.contains(&first)
        && second == Some(OPEN_PARENTHESIS)
        && let Some(end) = matching_close(rest, OPEN_PARENTHESIS, CLOSE_PARENTHESIS)
    {
        return Some((PieceKind::Extglob, end));
    }
    if SIMPLE_GLOBS.contains(&first) {
        return Some((PieceKind::Glob, first.len_utf8()));
    }
    if LONE_EXTGLOB_STARTS.contains(&first) {
        return Some((PieceKind::Literal, first.len_utf8()));
    }
    if first == OPEN_BRACKET {
        return Some(class_length(rest).map_or_else(
            || (PieceKind::Literal, first.len_utf8()),
            |length| (PieceKind::Glob, length),
        ));
    }
    if first == OPEN_BRACE {
        return brace_length(rest).map(|length| (PieceKind::Brace, length));
    }
    None
}

fn matching_close(rest: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, character) in rest.char_indices() {
        if character == open {
            depth = depth.saturating_add(1);
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index.saturating_add(character.len_utf8()));
            }
        }
    }
    None
}

// El `readClass` de `readGlob`: `[` con al menos un carácter antes del `]`, salvo que el primero sea
// el propio `]`.
fn class_length(rest: &str) -> Option<usize> {
    let after_open = rest.get(OPEN_BRACKET.len_utf8()..)?;
    let after_negation = after_open.strip_prefix(CLASS_NEGATIONS).unwrap_or(after_open);
    let after_leading = after_negation
        .strip_prefix(CLOSE_BRACKET)
        .unwrap_or(after_negation);
    let has_leading = after_leading.len() != after_negation.len();
    let close = after_leading.find(CLOSE_BRACKET)?;
    if close == 0 && !has_leading {
        return None;
    }
    let consumed = rest.len().saturating_sub(after_leading.len());
    Some(
        consumed
            .saturating_add(close)
            .saturating_add(CLOSE_BRACKET.len_utf8()),
    )
}

fn brace_length(rest: &str) -> Option<usize> {
    let end = matching_close(rest, OPEN_BRACE, CLOSE_BRACE)?;
    let inside = rest.get(OPEN_BRACE.len_utf8()..end.saturating_sub(CLOSE_BRACE.len_utf8()))?;
    (inside.contains(BRACE_LIST) || inside.contains(BRACE_RANGE)).then_some(end)
}
