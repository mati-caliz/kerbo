use std::borrow::Cow;
use std::mem;

use tree_sitter::Node;

use super::{CpdToken, Suppression, SuppressionState, image_hash, suppression};
use crate::syntax::one_based;

const ATOMIC_KINDS: &[&str] = &["string_literal", "character_literal"];
const COMMENT_KINDS: &[&str] = &["line_comment", "block_comment"];
const STRING_KIND: &str = "string_literal";
const SUPPRESSION_START: &str = "\"CPD-START\"";
const SUPPRESSION_END: &str = "\"CPD-END\"";
const SPLIT_TOKENS: &[(&str, &[&str])] = &[
    (">>", &[">", ">"]),
    (">>>", &[">", ">", ">"]),
    ("@interface", &["@", "interface"]),
    ("non-sealed", &["non", "-", "sealed"]),
];
const RESERVED_WORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Semicolon,
    PackageOrImport,
    At,
    OpenParenthesis,
    CloseParenthesis,
    Identifier,
    StringLiteral,
    Other,
}

struct Lexeme<'source> {
    text: Cow<'source, str>,
    kind: Kind,
    line: usize,
    end_line: usize,
    suppression: Suppression,
}

pub fn tokens(root: Node<'_>, source: &[u8]) -> Vec<CpdToken> {
    let mut filter = JavaFilter::default();
    let mut suppressing = SuppressionState::default();
    lexemes(root, source)
        .into_iter()
        .filter(|lexeme| {
            let discarded = filter.discards(lexeme);
            let suppressed = suppressing.apply(lexeme.suppression);
            !(suppressed || discarded)
        })
        .map(|lexeme| CpdToken {
            image: image_hash(&lexeme.text),
            line: lexeme.line,
            end_line: lexeme.end_line,
        })
        .collect()
}

fn lexemes<'source>(root: Node<'_>, source: &'source [u8]) -> Vec<Lexeme<'source>> {
    let mut found = Vec::new();
    let mut comments: Vec<&str> = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let kind = node.kind();
        if COMMENT_KINDS.contains(&kind) {
            comments.push(node.utf8_text(source).unwrap_or_default());
            continue;
        }
        if node.child_count() > 0 && !ATOMIC_KINDS.contains(&kind) {
            pending.extend(
                (0..node.child_count())
                    .rev()
                    .filter_map(|index| node.child(index)),
            );
            continue;
        }
        if node.start_byte() == node.end_byte() {
            continue;
        }
        let text = node.utf8_text(source).unwrap_or_default();
        let line = one_based(node.start_position().row);
        let end_line = one_based(node.end_position().row);
        let leading = suppression(mem::take(&mut comments).into_iter());
        for (piece, part) in split(text).into_iter().enumerate() {
            found.push(Lexeme {
                kind: classify(kind, &part),
                text: part,
                line,
                end_line,
                suppression: if piece == 0 {
                    leading
                } else {
                    Suppression::Unchanged
                },
            });
        }
    }
    found
}

fn split(text: &str) -> Vec<Cow<'_, str>> {
    SPLIT_TOKENS
        .iter()
        .find(|(joined, _)| *joined == text)
        .map_or_else(
            || vec![translate_unicode_escapes(text)],
            |(_, parts)| parts.iter().map(|&part| Cow::Borrowed(part)).collect(),
        )
}

fn classify(node_kind: &str, text: &str) -> Kind {
    match text {
        ";" => Kind::Semicolon,
        "package" | "import" => Kind::PackageOrImport,
        "@" => Kind::At,
        "(" => Kind::OpenParenthesis,
        ")" => Kind::CloseParenthesis,
        _ if node_kind == STRING_KIND => Kind::StringLiteral,
        _ if is_identifier(text) => Kind::Identifier,
        _ => Kind::Other,
    }
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '$')
        && characters.all(|character| character.is_alphanumeric() || character == '_' || character == '$')
        && !RESERVED_WORDS.contains(&text)
}

fn translate_unicode_escapes(text: &str) -> Cow<'_, str> {
    if !text.contains("\\u") {
        return Cow::Borrowed(text);
    }
    let mut translated = String::with_capacity(text.len());
    let mut characters = text.char_indices();
    let mut preceding_backslashes = 0_usize;
    while let Some((offset, character)) = characters.next() {
        let escape = (character == '\\' && preceding_backslashes.is_multiple_of(2))
            .then(|| unicode_escape(text.get(offset..).unwrap_or_default()))
            .flatten();
        if let Some((decoded, length)) = escape {
            translated.push(decoded);
            for _ in 1..length {
                characters.next();
            }
            preceding_backslashes = 0;
            continue;
        }
        preceding_backslashes = if character == '\\' {
            preceding_backslashes.saturating_add(1)
        } else {
            0
        };
        translated.push(character);
    }
    Cow::Owned(translated)
}

fn unicode_escape(text: &str) -> Option<(char, usize)> {
    let after_backslash = text.strip_prefix('\\')?;
    let digits = after_backslash.trim_start_matches('u');
    let marker_length = after_backslash.len().saturating_sub(digits.len());
    let hex = digits.get(..4).filter(|_| marker_length > 0)?;
    let decoded = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)?;
    Some((decoded, marker_length.saturating_add(5)))
}

#[derive(Debug, Default)]
struct AnnotationTracker {
    inside: bool,
    ends_with_next_token: bool,
    depth: i32,
}

impl AnnotationTracker {
    fn track(&mut self, kind: Kind) {
        if self.inside && self.ends_with_next_token {
            self.inside = false;
            self.ends_with_next_token = false;
        }
        if self.inside {
            self.follow(kind);
        }
        if kind == Kind::At {
            self.inside = true;
        }
    }

    const fn follow(&mut self, kind: Kind) {
        match kind {
            Kind::OpenParenthesis => self.depth = self.depth.saturating_add(1),
            Kind::CloseParenthesis => {
                self.depth = self.depth.saturating_sub(1);
                self.ends_with_next_token = self.depth == 0;
            }
            Kind::Identifier => {}
            Kind::Semicolon | Kind::PackageOrImport | Kind::At | Kind::StringLiteral | Kind::Other => {
                self.inside = self.depth != 0;
            }
        }
    }
}

#[derive(Debug, Default)]
struct JavaFilter {
    annotation: AnnotationTracker,
    discarding_keywords: bool,
    discarding_suppressed: bool,
}

impl JavaFilter {
    fn discards(&mut self, lexeme: &Lexeme<'_>) -> bool {
        self.annotation.track(lexeme.kind);
        if lexeme.kind == Kind::PackageOrImport {
            self.discarding_keywords = true;
        } else if self.discarding_keywords && lexeme.kind == Kind::Semicolon {
            self.discarding_keywords = false;
        }
        if self.annotation.inside && lexeme.kind == Kind::StringLiteral {
            if !self.discarding_suppressed && lexeme.text == SUPPRESSION_START {
                self.discarding_suppressed = true;
            } else if self.discarding_suppressed && lexeme.text == SUPPRESSION_END {
                self.discarding_suppressed = false;
            }
        }
        lexeme.kind == Kind::Semicolon
            || self.discarding_keywords
            || self.annotation.inside
            || self.discarding_suppressed
    }
}
