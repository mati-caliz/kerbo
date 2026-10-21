use tree_sitter::Node;

use super::syntax::{
    ANSI_STRING, ARITHMETIC_EXPANSION, COMMAND_SUBSTITUTION, CONCATENATION, EXPANSION, EXPANSIONS, NUMBER,
    RAW_STRING, SIMPLE_EXPANSION, SPECIAL_VARIABLE, STRING, STRING_CONTENT, VARIABLE_NAME, WORD, children,
    named_children, text,
};

const EXPANDED_PLACEHOLDER: &str = "${VAR}";
const ESCAPE: char = '\\';
const NEWLINE: char = '\n';
const GLOB_CHARACTERS: &[char] = &['*', '?'];
const EXTGLOB_STARTS: &[&str] = &["@(", "+(", "!("];
const OPEN_BRACKET: char = '[';
const CLOSE_BRACKET: char = ']';
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const BRACE_LIST: char = ',';
const BRACE_RANGE: &str = "..";
const DOUBLE_QUOTE_ESCAPABLE: &[char] = &['\\', '"', '$', '`'];
const BRACED_OPEN: &str = "${";
const SIMPLE_OPEN: &str = "$";
const SINGLE_QUOTE: char = '\'';
const ANSI_PREFIX: char = '$';

pub fn literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    let raw = text(node, source);
    match node.kind() {
        WORD => unquoted_literal(raw),
        NUMBER => Some(raw.to_owned()),
        STRING_CONTENT => Some(double_quoted(raw)),
        RAW_STRING => Some(raw.trim_matches(SINGLE_QUOTE).to_owned()),
        ANSI_STRING => Some(decode_ansi(
            raw.trim_start_matches(ANSI_PREFIX).trim_matches(SINGLE_QUOTE),
        )),
        STRING => named_children(node)
            .into_iter()
            .map(|part| (part.kind() == STRING_CONTENT).then(|| double_quoted(text(part, source))))
            .collect(),
        CONCATENATION => {
            let joined: Option<String> = children(node)
                .into_iter()
                .map(|part| literal(part, source))
                .collect();
            joined.filter(|_| !is_brace_expansion(node, source))
        }
        _ => None,
    }
}

pub fn literal_or(node: Node<'_>, source: &[u8], fallback: &str) -> String {
    match node.kind() {
        CONCATENATION => children(node)
            .into_iter()
            .map(|part| literal_or(part, source, fallback))
            .collect(),
        STRING => named_children(node)
            .into_iter()
            .map(|part| {
                if part.kind() == STRING_CONTENT {
                    double_quoted(text(part, source))
                } else {
                    fallback.to_owned()
                }
            })
            .collect(),
        _ => literal(node, source).unwrap_or_else(|| fallback.to_owned()),
    }
}

pub fn oversimplify(node: Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        WORD => unescaped(text(node, source)),
        NUMBER => text(node, source).to_owned(),
        RAW_STRING => text(node, source).trim_matches(SINGLE_QUOTE).to_owned(),
        ANSI_STRING => text(node, source)
            .trim_start_matches(ANSI_PREFIX)
            .trim_matches(SINGLE_QUOTE)
            .to_owned(),
        STRING | CONCATENATION => named_children(node)
            .into_iter()
            .map(|part| {
                if part.kind() == STRING_CONTENT {
                    double_quoted(text(part, source))
                } else {
                    oversimplify(part, source)
                }
            })
            .collect(),
        SIMPLE_EXPANSION | EXPANSION | COMMAND_SUBSTITUTION | ARITHMETIC_EXPANSION => {
            EXPANDED_PLACEHOLDER.to_owned()
        }
        _ => String::new(),
    }
}

pub fn parts(word: Node<'_>) -> Vec<Node<'_>> {
    match word.kind() {
        CONCATENATION => children(word).into_iter().flat_map(parts).collect(),
        STRING => named_children(word),
        _ => vec![word],
    }
}

pub fn braced_content(expansion: Node<'_>, source: &[u8]) -> String {
    let full = text(expansion, source);
    let leading = full
        .len()
        .saturating_sub(super::syntax::token_text(expansion, source).len());
    let raw = super::syntax::token_text(expansion, source);
    if expansion.kind() == SIMPLE_EXPANSION {
        return raw.strip_prefix(SIMPLE_OPEN).unwrap_or(raw).to_owned();
    }
    let start = expansion
        .start_byte()
        .saturating_add(leading)
        .saturating_add(BRACED_OPEN.len());
    let end = expansion.end_byte().saturating_sub(CLOSE_BRACE.len_utf8());
    render(expansion, source, start, end)
}

fn render(node: Node<'_>, source: &[u8], start: usize, end: usize) -> String {
    let mut rendered = String::new();
    let mut cursor = start;
    for child in children(node) {
        if child.end_byte() <= start || child.start_byte() >= end {
            continue;
        }
        rendered.push_str(slice(source, cursor, child.start_byte()));
        let nested = EXPANSIONS.contains(&child.kind())
            || [COMMAND_SUBSTITUTION, ARITHMETIC_EXPANSION].contains(&child.kind());
        if nested {
            rendered.push_str(EXPANDED_PLACEHOLDER);
        } else if [STRING, RAW_STRING].contains(&child.kind()) {
            rendered.push_str(&oversimplify(child, source));
        } else if child.child_count() == 0 || [VARIABLE_NAME, SPECIAL_VARIABLE].contains(&child.kind()) {
            rendered.push_str(slice(source, child.start_byte(), child.end_byte()));
        } else {
            rendered.push_str(&render(child, source, child.start_byte(), child.end_byte()));
        }
        cursor = child.end_byte();
    }
    rendered.push_str(slice(source, cursor, end));
    rendered
}

fn slice(source: &[u8], start: usize, end: usize) -> &str {
    source
        .get(start..end.max(start))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or_default()
}

fn unquoted_literal(raw: &str) -> Option<String> {
    let mut literal = String::new();
    let mut characters = raw.chars();
    let mut open_bracket = false;
    while let Some(character) = characters.next() {
        if character == ESCAPE {
            if let Some(escaped) = characters.next().filter(|escaped| *escaped != NEWLINE) {
                literal.push(escaped);
            }
            continue;
        }
        if GLOB_CHARACTERS.contains(&character) || (open_bracket && character == CLOSE_BRACKET) {
            return None;
        }
        open_bracket |= character == OPEN_BRACKET;
        literal.push(character);
    }
    let globbed = EXTGLOB_STARTS.iter().any(|start| raw.contains(start));
    (!globbed && !has_brace_expansion(raw)).then_some(literal)
}

pub fn is_brace_expansion(node: Node<'_>, source: &[u8]) -> bool {
    let unquoted: String = children(node)
        .into_iter()
        .filter(|part| part.kind() == WORD)
        .map(|part| text(part, source))
        .collect();
    has_brace_expansion(&unquoted)
}

fn has_brace_expansion(raw: &str) -> bool {
    raw.split(OPEN_BRACE).skip(1).any(|after| {
        after
            .split_once(CLOSE_BRACE)
            .is_some_and(|(inside, _)| inside.contains(BRACE_LIST) || inside.contains(BRACE_RANGE))
    })
}

fn unescaped(raw: &str) -> String {
    let mut plain = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character == ESCAPE {
            if let Some(escaped) = characters.next().filter(|escaped| *escaped != NEWLINE) {
                plain.push(escaped);
            }
        } else {
            plain.push(character);
        }
    }
    plain
}

fn double_quoted(raw: &str) -> String {
    let mut plain = String::new();
    let mut characters = raw.chars().peekable();
    while let Some(character) = characters.next() {
        match characters.peek().copied() {
            Some(next) if character == ESCAPE && DOUBLE_QUOTE_ESCAPABLE.contains(&next) => {
                plain.push(next);
                characters.next();
            }
            Some(NEWLINE) if character == ESCAPE => {
                characters.next();
            }
            _ => plain.push(character),
        }
    }
    plain
}

fn decode_ansi(raw: &str) -> String {
    let mut decoded = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character != ESCAPE {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => decoded.push('\n'),
            Some('t') => decoded.push('\t'),
            Some('r') => decoded.push('\r'),
            Some(other) => decoded.push(other),
            None => decoded.push(ESCAPE),
        }
    }
    decoded
}
