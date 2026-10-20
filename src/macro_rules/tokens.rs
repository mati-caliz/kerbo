use unicode_ident::{is_xid_continue, is_xid_start};

const COMPOUND_PUNCTUATION: &[&str] = &[
    "<<=", ">>=", "...", "..=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=",
    "%=", "^=", "&=", "|=", "<<", ">>", "..",
];
const LINE_COMMENT: &str = "//";
const BLOCK_COMMENT_START: &str = "/*";
const BLOCK_COMMENT_END: &str = "*/";
const RAW_IDENTIFIER: &str = "r#";
const LITERAL_PREFIXES: &[&str] = &["br", "cr", "b", "c", "r"];
const QUOTE: char = '"';
const APOSTROPHE: char = '\'';
const ESCAPE: char = '\\';
const HASH: char = '#';
const DOT: char = '.';
const RADIX_PREFIXES: &[&str] = &["0x", "0o", "0b"];
const EXPONENTS: &[char] = &['e', 'E'];
const SIGNS: &[char] = &['+', '-'];
const DIGIT_SEPARATOR: char = '_';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Origin {
    pub source: usize,
    pub offset: usize,
    pub expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Parenthesis,
    Bracket,
    Brace,
    Invisible,
}

impl Delimiter {
    const fn opened_by(character: char) -> Option<Self> {
        match character {
            '(' => Some(Self::Parenthesis),
            '[' => Some(Self::Bracket),
            '{' => Some(Self::Brace),
            _ => None,
        }
    }

    const fn closed_by(character: char) -> Option<Self> {
        match character {
            ')' => Some(Self::Parenthesis),
            ']' => Some(Self::Bracket),
            '}' => Some(Self::Brace),
            _ => None,
        }
    }

    pub const fn marks(self) -> (&'static str, &'static str) {
        match self {
            Self::Parenthesis | Self::Invisible => ("(", ")"),
            Self::Bracket => ("[", "]"),
            Self::Brace => ("{", "}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    Ident,
    Lifetime,
    Literal,
    Punct,
}

#[derive(Debug, Clone)]
pub struct Leaf {
    pub kind: LeafKind,
    pub text: String,
    pub origin: Origin,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub delimiter: Delimiter,
    pub open: Origin,
    pub close: Origin,
    pub trees: Vec<TokenTree>,
}

#[derive(Debug, Clone)]
pub enum TokenTree {
    Leaf(Leaf),
    Group(Group),
}

impl TokenTree {
    pub fn is_punct(&self, mark: &str) -> bool {
        matches!(self, Self::Leaf(leaf) if leaf.kind == LeafKind::Punct && leaf.text == mark)
    }

    pub fn is_word(&self, word: &str) -> bool {
        matches!(self, Self::Leaf(leaf) if leaf.kind == LeafKind::Ident && leaf.text == word)
    }

    pub const fn leaf(&self) -> Option<&Leaf> {
        match self {
            Self::Leaf(leaf) => Some(leaf),
            Self::Group(_) => None,
        }
    }

    pub const fn group(&self) -> Option<&Group> {
        match self {
            Self::Group(group) => Some(group),
            Self::Leaf(_) => None,
        }
    }

    pub fn delimited_by(&self, delimiter: Delimiter) -> bool {
        self.group().is_some_and(|group| group.delimiter == delimiter)
    }

    pub const fn first_origin(&self) -> Origin {
        match self {
            Self::Leaf(leaf) => leaf.origin,
            Self::Group(group) => group.open,
        }
    }

    pub const fn last_origin(&self) -> Origin {
        match self {
            Self::Leaf(leaf) => leaf.origin,
            Self::Group(group) => group.close,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Raw {
    Leaf(LeafKind, usize, usize),
    Open(Delimiter, usize),
    Close(Delimiter, usize),
}

struct OpenGroup {
    delimiter: Delimiter,
    open: Origin,
    trees: Vec<TokenTree>,
}

pub fn lex(text: &str, locate: &dyn Fn(usize) -> Origin) -> Option<Vec<TokenTree>> {
    let mut stack = vec![OpenGroup {
        delimiter: Delimiter::Invisible,
        open: locate(0),
        trees: Vec::new(),
    }];
    let mut position = 0;
    let mut after_dot = false;
    while let Some((raw, next)) = next_raw(text, skip_trivia(text, position), after_dot) {
        after_dot = matches!(raw, Raw::Leaf(LeafKind::Punct, start, _) if text.get(start..).is_some_and(|rest| rest.starts_with(DOT)));
        push_raw(text, raw, &mut stack, locate)?;
        position = next;
    }
    let root = stack.pop()?;
    stack.is_empty().then_some(root.trees)
}

fn push_raw(
    text: &str,
    raw: Raw,
    stack: &mut Vec<OpenGroup>,
    locate: &dyn Fn(usize) -> Origin,
) -> Option<()> {
    match raw {
        Raw::Leaf(kind, start, end) => {
            stack.last_mut()?.trees.push(TokenTree::Leaf(Leaf {
                kind,
                text: text.get(start..end)?.to_owned(),
                origin: locate(start),
            }));
        }
        Raw::Open(delimiter, start) => stack.push(OpenGroup {
            delimiter,
            open: locate(start),
            trees: Vec::new(),
        }),
        Raw::Close(delimiter, start) => {
            let closed = stack.pop().filter(|open| open.delimiter == delimiter)?;
            stack.last_mut()?.trees.push(TokenTree::Group(Group {
                delimiter,
                open: closed.open,
                close: locate(start),
                trees: closed.trees,
            }));
        }
    }
    Some(())
}

fn skip_trivia(text: &str, mut position: usize) -> usize {
    loop {
        let Some(rest) = text.get(position..) else {
            return position;
        };
        let trimmed = rest.trim_start();
        position = position.saturating_add(rest.len().saturating_sub(trimmed.len()));
        if trimmed.starts_with(LINE_COMMENT) && !trimmed.starts_with(BLOCK_COMMENT_START) {
            position = position.saturating_add(trimmed.find('\n').unwrap_or(trimmed.len()));
        } else if trimmed.starts_with(BLOCK_COMMENT_START) {
            position = position.saturating_add(block_comment_length(trimmed));
        } else {
            return position;
        }
    }
}

fn block_comment_length(text: &str) -> usize {
    let mut depth = 0_usize;
    let mut position = 0;
    while let Some(rest) = text.get(position..).filter(|rest| !rest.is_empty()) {
        if rest.starts_with(BLOCK_COMMENT_START) {
            depth = depth.saturating_add(1);
            position = position.saturating_add(BLOCK_COMMENT_START.len());
        } else if rest.starts_with(BLOCK_COMMENT_END) {
            depth = depth.saturating_sub(1);
            position = position.saturating_add(BLOCK_COMMENT_END.len());
            if depth == 0 {
                return position;
            }
        } else {
            position = position.saturating_add(rest.chars().next().map_or(1, char::len_utf8));
        }
    }
    text.len()
}

fn next_raw(text: &str, start: usize, after_dot: bool) -> Option<(Raw, usize)> {
    let rest = text.get(start..)?;
    let first = rest.chars().next()?;
    let leaf = |kind: LeafKind, length: usize| {
        let end = start.saturating_add(length);
        (Raw::Leaf(kind, start, end), end)
    };
    if let Some(delimiter) = Delimiter::opened_by(first) {
        return Some((Raw::Open(delimiter, start), start.saturating_add(1)));
    }
    if let Some(delimiter) = Delimiter::closed_by(first) {
        return Some((Raw::Close(delimiter, start), start.saturating_add(1)));
    }
    if let Some(length) = prefixed_literal(rest) {
        return Some(leaf(LeafKind::Literal, length));
    }
    if is_identifier_start(first) || rest.starts_with(RAW_IDENTIFIER) {
        return Some(leaf(LeafKind::Ident, identifier_length(rest)));
    }
    Some(match first {
        QUOTE => leaf(LeafKind::Literal, quoted_length(rest, QUOTE)),
        APOSTROPHE => {
            apostrophe(rest).map_or_else(|| leaf(LeafKind::Punct, 1), |(kind, length)| leaf(kind, length))
        }
        _ if first.is_ascii_digit() => leaf(LeafKind::Literal, number_length(rest, after_dot)),
        _ => leaf(LeafKind::Punct, punctuation_length(rest, first)),
    })
}

fn is_identifier_start(character: char) -> bool {
    character == DIGIT_SEPARATOR || is_xid_start(character)
}

fn identifier_length(text: &str) -> usize {
    let body = text.strip_prefix(RAW_IDENTIFIER).unwrap_or(text);
    let prefix = text.len().saturating_sub(body.len());
    prefix.saturating_add(
        body.char_indices()
            .find(|&(_, character)| !is_xid_continue(character))
            .map_or(body.len(), |(index, _)| index),
    )
}

fn prefixed_literal(text: &str) -> Option<usize> {
    LITERAL_PREFIXES.iter().find_map(|prefix| {
        let rest = text.strip_prefix(prefix)?;
        let hashes = rest.len().saturating_sub(rest.trim_start_matches(HASH).len());
        let is_raw = prefix.ends_with('r');
        let body = rest.get(hashes..)?;
        let length = if is_raw && body.starts_with(QUOTE) {
            raw_string_length(body, hashes)
        } else if hashes == 0 && body.starts_with(QUOTE) {
            quoted_length(body, QUOTE)
        } else if hashes == 0 && !is_raw && body.starts_with(APOSTROPHE) && prefix == &"b" {
            quoted_length(body, APOSTROPHE)
        } else {
            return None;
        };
        Some(prefix.len().saturating_add(hashes).saturating_add(length))
    })
}

fn raw_string_length(body: &str, hashes: usize) -> usize {
    let closing: String = std::iter::once(QUOTE)
        .chain(std::iter::repeat_n(HASH, hashes))
        .collect();
    body.get(1..)
        .and_then(|inside| inside.find(&closing))
        .map_or(body.len(), |end| {
            end.saturating_add(1).saturating_add(closing.len())
        })
}

fn quoted_length(text: &str, quote: char) -> usize {
    let mut escaped = false;
    for (index, character) in text.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == ESCAPE {
            escaped = true;
        } else if character == quote {
            return index.saturating_add(quote.len_utf8());
        }
    }
    text.len()
}

fn apostrophe(text: &str) -> Option<(LeafKind, usize)> {
    let mut characters = text.char_indices().skip(1);
    let (_, second) = characters.next()?;
    if second == ESCAPE {
        return Some((LeafKind::Literal, quoted_length(text, APOSTROPHE)));
    }
    if let Some((index, APOSTROPHE)) = characters.next() {
        return Some((LeafKind::Literal, index.saturating_add(1)));
    }
    is_identifier_start(second).then(|| {
        let name = text.get(1..).unwrap_or_default();
        (LeafKind::Lifetime, identifier_length(name).saturating_add(1))
    })
}

fn number_length(text: &str, after_dot: bool) -> usize {
    if RADIX_PREFIXES.iter().any(|prefix| text.starts_with(prefix)) {
        return identifier_length(text.get(1..).unwrap_or_default()).saturating_add(1);
    }
    let digits = |from: usize| {
        text.get(from..).map_or(from, |rest| {
            from.saturating_add(
                rest.find(|character: char| !character.is_ascii_digit() && character != DIGIT_SEPARATOR)
                    .unwrap_or(rest.len()),
            )
        })
    };
    let mut end = digits(0);
    let after = |position: usize| text.get(position..).unwrap_or_default();
    if !after_dot
        && after(end).starts_with(DOT)
        && after(end)
            .chars()
            .nth(1)
            .is_some_and(|next| next.is_ascii_digit())
    {
        end = digits(end.saturating_add(1));
    }
    let rest = after(end);
    let mut exponent = rest.chars();
    if exponent.next().is_some_and(|mark| EXPONENTS.contains(&mark)) {
        let sign = exponent.clone().next().is_some_and(|mark| SIGNS.contains(&mark));
        let digit_at = if sign { 2 } else { 1 };
        if rest
            .chars()
            .nth(digit_at)
            .is_some_and(|digit| digit.is_ascii_digit())
        {
            end = digits(end.saturating_add(digit_at));
        }
    }
    let suffix = after(end);
    end.saturating_add(
        suffix
            .char_indices()
            .find(|&(_, character)| !is_xid_continue(character))
            .map_or(suffix.len(), |(index, _)| index),
    )
}

fn punctuation_length(text: &str, first: char) -> usize {
    COMPOUND_PUNCTUATION
        .iter()
        .find(|compound| text.starts_with(*compound))
        .map_or_else(|| first.len_utf8(), |compound| compound.len())
}
