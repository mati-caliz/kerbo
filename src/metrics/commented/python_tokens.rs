use unicode_ident::{is_xid_continue, is_xid_start};

const STRING_PREFIXES: &[&str] = &[
    "B", "BR", "Br", "F", "FR", "Fr", "R", "RB", "RF", "Rb", "Rf", "U", "b", "bR", "br", "f", "fR", "fr",
    "r", "rB", "rF", "rb", "rf", "u", "T", "TR", "Tr", "RT", "Rt", "t", "tR", "tr", "rT", "rt",
];
const KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
    "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lazy", "lambda", "nonlocal",
    "not", "or", "pass", "raise", "return", "try", "while", "match", "type", "case", "with", "yield",
];
const THREE_CHARACTER_OPERATORS: &[&str] = &["**=", "//=", "<<=", ">>=", "..."];
const TWO_CHARACTER_OPERATORS: &[&str] = &[
    "==", "+=", "*=", "**", "/=", "//", "%=", "|=", "^=", "&=", "-=", "->", "@=", "!=", ":=", "<<", "<=",
    ">>", ">=",
];
const SINGLE_CHARACTER_OPERATORS: &[char] = &[
    '=', '+', '*', '/', '%', '|', '^', '&', '-', '@', '~', ':', ';', '<', '>', ',', '.', '(', ')', '[', ']',
    '{', '}',
];
const TRIVIA: &[char] = &[' ', '\t', '\u{c}'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Name,
    Keyword,
    Punctuation,
    Other,
}

fn is_identifier_start(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphabetic() || character == '_'
    } else {
        is_xid_start(character)
    }
}

fn is_identifier_continuation(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphanumeric() || character == '_'
    } else {
        is_xid_continue(character)
    }
}

// Calca el SimpleTokenizer de ruff: lo primero que no reconoce corta el resto en un token "bogus",
// y la pregunta es sólo si aparecen dos nombres seguidos (prosa, no código).
pub fn has_consecutive_names(text: &str) -> bool {
    let mut previous_was_name = false;
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let (kind, length) = next_token(rest, first);
        if kind == Some(Kind::Other) {
            return false;
        }
        rest = rest.get(length..).unwrap_or_default();
        let Some(kind) = kind else {
            continue;
        };
        let is_name = kind == Kind::Name;
        if is_name && previous_was_name {
            return true;
        }
        previous_was_name = is_name;
    }
    false
}

fn next_token(rest: &str, first: char) -> (Option<Kind>, usize) {
    if is_identifier_start(first) {
        return identifier(rest);
    }
    if TRIVIA.contains(&first) {
        let length = rest
            .find(|character| !TRIVIA.contains(&character))
            .unwrap_or(rest.len());
        return (None, length);
    }
    match first {
        '\n' | '\r' | '\\' => (None, first.len_utf8()),
        '#' => (None, rest.find(['\n', '\r']).unwrap_or(rest.len())),
        _ => match operator_length(rest) {
            0 => (Some(Kind::Other), first.len_utf8()),
            length => (Some(Kind::Punctuation), length),
        },
    }
}

fn identifier(rest: &str) -> (Option<Kind>, usize) {
    let length = rest
        .char_indices()
        .find(|&(_, character)| !is_identifier_continuation(character))
        .map_or(rest.len(), |(offset, _)| offset);
    let word = rest.get(..length).unwrap_or_default();
    let followed_by_quote = rest
        .get(length..)
        .is_some_and(|after| after.starts_with(['"', '\'']));
    if KEYWORDS.contains(&word) {
        return (Some(Kind::Keyword), length);
    }
    if followed_by_quote && STRING_PREFIXES.contains(&word) {
        return (Some(Kind::Other), length);
    }
    (Some(Kind::Name), length)
}

fn operator_length(rest: &str) -> usize {
    let longest = THREE_CHARACTER_OPERATORS
        .iter()
        .chain(TWO_CHARACTER_OPERATORS)
        .find(|operator| rest.starts_with(*operator))
        .map(|operator| operator.len());
    longest.unwrap_or_else(|| {
        rest.chars()
            .next()
            .filter(|character| SINGLE_CHARACTER_OPERATORS.contains(character))
            .map_or(0, char::len_utf8)
    })
}
