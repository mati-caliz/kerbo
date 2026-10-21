mod regex;

use tree_sitter::{Node, Point};

use super::Context;
use super::nodes::{ARGUMENTS_FIELD, CALL, STRING, TEMPLATE_STRING, TEMPLATE_SUBSTITUTION};
use crate::fix::Fix;
use crate::lint::{LintFinding, LintKind, finding_at};
use crate::rules::EslintRule;

const BACKSLASH: char = '\\';
const VALID_STRING_ESCAPES: &[char] = &[
    '\\', 'n', 'r', 'v', 't', 'b', 'f', 'u', 'x', '\n', '\r', '\u{2028}', '\u{2029}',
];
const TEMPLATE_TYPE: &str = "template_literal_type";
const TEMPLATE_HOLE: &str = "template_type";
const JSX_ATTRIBUTE: &str = "jsx_attribute";
const BACKTICK: char = '`';
const DOLLAR: char = '$';
const OPENING_BRACE: char = '{';
const REGEX: &str = "regex";

pub fn useless_escapes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for node in context.of_kind(&[STRING, TEMPLATE_STRING, TEMPLATE_TYPE, REGEX]) {
        let offsets = match node.kind() {
            STRING => string_escapes(node, context),
            REGEX => regex::useless_escapes(node, context.source),
            _ => template_escapes(node, context),
        };
        findings.extend(
            offsets
                .into_iter()
                .map(|(offset, escaped)| removal(node, offset, &escaped, context.source)),
        );
    }
    findings
}

fn string_escapes(node: Node<'_>, context: &Context<'_, '_>) -> Vec<(usize, String)> {
    if node.parent().is_some_and(|parent| parent.kind() == JSX_ATTRIBUTE) {
        return Vec::new();
    }
    let raw = context.text(node);
    let quote = raw.chars().next();
    escapes(raw)
        .into_iter()
        .filter(|(_, escaped)| !VALID_STRING_ESCAPES.contains(escaped) && Some(*escaped) != quote)
        .map(|(offset, escaped)| (offset, escaped.to_string()))
        .collect()
}

fn template_escapes(node: Node<'_>, context: &Context<'_, '_>) -> Vec<(usize, String)> {
    let is_tagged = node.parent().is_some_and(|parent| {
        parent.kind() == CALL
            && parent
                .child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| arguments.id() == node.id())
    });
    if is_tagged {
        return Vec::new();
    }
    let start = node.start_byte();
    let raw = context.text(node);
    let mut found = Vec::new();
    for (from, to) in element_spans(node) {
        let span = raw
            .get(from.saturating_sub(start)..to.saturating_sub(start))
            .unwrap_or_default();
        for (offset, escaped) in escapes(span) {
            let unnecessary = match escaped {
                DOLLAR => span
                    .get(offset.saturating_add(2)..)
                    .is_none_or(|rest| !rest.starts_with(OPENING_BRACE)),
                OPENING_BRACE => {
                    offset == 0 || !span.get(..offset).is_some_and(|before| before.ends_with(DOLLAR))
                }
                other => !VALID_STRING_ESCAPES.contains(&other),
            };
            if unnecessary && escaped != BACKTICK {
                found.push((
                    from.saturating_sub(start).saturating_add(offset),
                    escaped.to_string(),
                ));
            }
        }
    }
    found
}

fn element_spans(node: Node<'_>) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = node.start_byte();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if [TEMPLATE_SUBSTITUTION, TEMPLATE_HOLE].contains(&child.kind()) {
            spans.push((from, child.start_byte().saturating_add(2)));
            from = child.end_byte().saturating_sub(1);
        }
    }
    spans.push((from, node.end_byte()));
    spans
}

fn escapes(raw: &str) -> Vec<(usize, char)> {
    let mut found = Vec::new();
    let mut characters = raw.char_indices().peekable();
    while let Some((offset, character)) = characters.next() {
        if character != BACKSLASH {
            continue;
        }
        let Some(&(_, escaped)) = characters.peek() else {
            break;
        };
        if escaped.is_ascii_digit() {
            continue;
        }
        characters.next();
        found.push((offset, escaped));
    }
    found
}

pub fn point_at(node: Node<'_>, offset: usize, source: &[u8]) -> Point {
    let start = node.start_position();
    let prefix = source
        .get(node.start_byte()..node.start_byte().saturating_add(offset))
        .unwrap_or_default();
    let lines = prefix.split(|byte| *byte == b'\n').count().saturating_sub(1);
    prefix.iter().rposition(|byte| *byte == b'\n').map_or_else(
        || Point {
            row: start.row,
            column: start.column.saturating_add(offset),
        },
        |last| Point {
            row: start.row.saturating_add(lines),
            column: offset.saturating_sub(last).saturating_sub(1),
        },
    )
}

fn removal(node: Node<'_>, offset: usize, escaped: &str, source: &[u8]) -> LintFinding {
    let start = point_at(node, offset, source);
    let end = Point {
        row: start.row,
        column: start.column.saturating_add(1),
    };
    let fix = Fix::replace_range(start, end, "", source);
    finding_at(
        LintKind::Eslint(EslintRule::NoUselessEscape),
        start,
        escaped,
        Some(fix),
    )
}
