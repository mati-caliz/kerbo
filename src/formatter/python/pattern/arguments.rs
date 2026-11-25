use super::fmt_pattern;
use crate::formatter::python::ast::{NodeId, Other};
use crate::formatter::python::builders::{CommaSeparated, group};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::format_node_with;
use crate::formatter::python::parentheses::{Parentheses, empty_parenthesized, parenthesized};
use crate::formatter::python::trivia::{TokenKind, Tokenizer};

const PATTERN: &str = "patrón";
const EQUALS: &str = "=";

fn is_single_argument_parenthesized(f: &Formatter<'_>, pattern: NodeId, call_end: usize) -> bool {
    let start = f.ast.end(pattern);
    let mut has_seen_closing = false;
    let text = f.source.get(..call_end).unwrap_or_default();
    for token in Tokenizer::starts_at(start, text).skip_trivia() {
        if token.kind == TokenKind::RParen {
            if has_seen_closing {
                return true;
            }
            has_seen_closing = true;
        } else if token.kind != TokenKind::Comma {
            break;
        }
    }
    false
}

fn fmt_keyword(f: &mut Formatter<'_>, keyword: NodeId) -> Formatted {
    let Some(Other::PatternKeyword { attr, pattern }) = f.ast.other(keyword) else {
        return Err(f.unsupported(keyword, PATTERN));
    };
    format_node_with(f, keyword, |nested| {
        nested.source_text(*attr);
        nested.token(EQUALS);
        fmt_pattern(nested, *pattern, Parentheses::Preserve)
    })
}

fn fmt_all(f: &mut Formatter<'_>, node: NodeId, (patterns, keywords): (&[NodeId], &[NodeId])) -> Formatted {
    let end = f.ast.end(node);
    let mut joiner = CommaSeparated::new(end);
    if let ([only], []) = (patterns, keywords) {
        let parentheses = if is_single_argument_parenthesized(f, *only, end) {
            Parentheses::Always
        } else {
            Parentheses::Never
        };
        let entry_end = f.ast.end(*only);
        joiner.entry(f, entry_end, |entry| fmt_pattern(entry, *only, parentheses))?;
    } else {
        for pattern in patterns {
            let entry_end = f.ast.end(*pattern);
            joiner.entry(f, entry_end, |entry| {
                fmt_pattern(entry, *pattern, Parentheses::Preserve)
            })?;
        }
        for keyword in keywords {
            let entry_end = f.ast.end(*keyword);
            joiner.entry(f, entry_end, |entry| fmt_keyword(entry, *keyword))?;
        }
    }
    joiner.finish(f);
    Ok(())
}

pub fn fmt_arguments(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::PatternArguments { patterns, keywords }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PATTERN));
    };
    let dangling = f.comments.dangling(node);
    if patterns.is_empty() && keywords.is_empty() {
        return empty_parenthesized(f, ("(", ")"), dangling);
    }
    parenthesized(f, ("(", ")"), dangling, |nested| {
        group(nested, |inner| fmt_all(inner, node, (patterns, keywords)))
    })
}
