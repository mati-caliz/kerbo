use std::iter::Peekable;
use std::str::Chars;

use tree_sitter::Node;

use super::ast::{STRING, is_bytes, is_f_string, named, string_prefix, text};

const BACKSLASH: char = '\\';
const NAMED: char = 'N';
const NAME_OPENING: char = '{';
const NAME_CLOSING: char = '}';
const LINE_FEED: char = '\n';
const HEX: u32 = 16;
const OCTAL: u32 = 8;
const MAX_OCTAL_DIGITS: usize = 3;
const BYTE_DIGITS: usize = 2;
const SHORT_UNICODE_DIGITS: usize = 4;
const LONG_UNICODE_DIGITS: usize = 8;
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const QUOTES: [&str; 2] = ["\"", "'"];
const RAW: [char; 2] = ['r', 'R'];
const SIMPLE: [(char, char); 10] = [
    ('\\', '\\'),
    ('\'', '\''),
    ('"', '"'),
    ('n', '\n'),
    ('t', '\t'),
    ('r', '\r'),
    ('a', '\u{7}'),
    ('b', '\u{8}'),
    ('f', '\u{c}'),
    ('v', '\u{b}'),
];

pub fn decoded(content: &str) -> String {
    let mut decoded = String::with_capacity(content.len());
    let mut characters = content.chars().peekable();
    while let Some(character) = characters.next() {
        if character == BACKSLASH {
            decoded.push_str(&escape(&mut characters));
        } else {
            decoded.push(character);
        }
    }
    decoded
}

fn escape(characters: &mut Peekable<Chars<'_>>) -> String {
    let Some(escaped) = characters.next() else {
        return BACKSLASH.to_string();
    };
    if escaped == LINE_FEED {
        return String::new();
    }
    if let Some((_, value)) = SIMPLE.iter().find(|(name, _)| *name == escaped) {
        return value.to_string();
    }
    if escaped == NAMED && characters.next_if_eq(&NAME_OPENING).is_some() {
        let name: String = characters
            .by_ref()
            .take_while(|character| *character != NAME_CLOSING)
            .collect();
        return format!("{BACKSLASH}{NAMED}<{name}>");
    }
    let (radix, limit, mut digits) = match escaped {
        'x' => (HEX, BYTE_DIGITS, String::new()),
        'u' => (HEX, SHORT_UNICODE_DIGITS, String::new()),
        'U' => (HEX, LONG_UNICODE_DIGITS, String::new()),
        octal if octal.is_digit(OCTAL) => (OCTAL, MAX_OCTAL_DIGITS, octal.to_string()),
        _ => return format!("{BACKSLASH}{escaped}"),
    };
    while digits.len() < limit
        && let Some(next) = characters.next_if(|next| next.is_digit(radix))
    {
        digits.push(next);
    }
    u32::from_str_radix(&digits, radix)
        .ok()
        .and_then(char::from_u32)
        .map_or_else(
            || format!("{BACKSLASH}{escaped}{}", digits.trim_start_matches(escaped)),
            |value| value.to_string(),
        )
}

pub fn text_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = if node.kind() == STRING {
        vec![node]
    } else {
        named(node)
    };
    let plain = parts
        .iter()
        .all(|part| part.kind() == STRING && !is_f_string(*part, source) && !is_bytes(*part, source));
    plain.then(|| parts.iter().map(|part| part_value(*part, source)).collect())
}

pub fn part_value(part: Node<'_>, source: &[u8]) -> String {
    let prefix = string_prefix(part, source);
    let literal = text(part, source).get(prefix.len()..).unwrap_or_default();
    let quote = TRIPLE_QUOTES
        .iter()
        .chain(QUOTES.iter())
        .find(|quote| literal.starts_with(**quote))
        .map_or(0, |quote| quote.len());
    let content = literal
        .get(quote..literal.len().saturating_sub(quote))
        .unwrap_or_default();
    if prefix.contains(RAW) {
        content.to_owned()
    } else {
        decoded(content)
    }
}
