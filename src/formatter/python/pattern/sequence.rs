use super::fmt_pattern;
use crate::formatter::python::ast::{NodeId, Other, SequenceKind};
use crate::formatter::python::builders::{CommaSeparated, group};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::format_expr_preserving;
use crate::formatter::python::parentheses::{
    Parentheses, empty_parenthesized, optional_parentheses, parenthesized,
};

const PATTERN: &str = "patrón";
const COLON: &str = ":";
const DOUBLE_STAR: &str = "**";
const COMMA: &str = ",";
const DICT_COMMENT: &str = "comentario dentro de un patrón de diccionario con resto";

fn fmt_entries(f: &mut Formatter<'_>, node: NodeId, patterns: &[NodeId]) -> Formatted {
    let mut joiner = CommaSeparated::new(f.ast.end(node));
    for pattern in patterns {
        let end = f.ast.end(*pattern);
        joiner.entry(f, end, |entry| {
            fmt_pattern(entry, *pattern, Parentheses::Preserve)
        })?;
    }
    joiner.finish(f);
    Ok(())
}

pub fn fmt_sequence(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::PatternSequence { patterns, kind }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PATTERN));
    };
    let dangling = f.comments.dangling(node);
    let brackets = match kind {
        SequenceKind::List => ("[", "]"),
        SequenceKind::Tuple | SequenceKind::TupleWithoutParentheses => ("(", ")"),
    };
    match (patterns.as_slice(), kind) {
        ([], _) => empty_parenthesized(f, brackets, dangling),
        ([only], SequenceKind::Tuple | SequenceKind::TupleWithoutParentheses) => {
            parenthesized(f, brackets, dangling, |nested| {
                fmt_pattern(nested, *only, Parentheses::Preserve)?;
                nested.token(COMMA);
                Ok(())
            })
        }
        (_, SequenceKind::TupleWithoutParentheses) => {
            optional_parentheses(f, |nested| fmt_entries(nested, node, patterns))
        }
        (_, SequenceKind::List | SequenceKind::Tuple) => parenthesized(f, brackets, dangling, |nested| {
            fmt_entries(nested, node, patterns)
        }),
    }
}

fn fmt_pair(f: &mut Formatter<'_>, key: NodeId, pattern: NodeId) -> Formatted {
    group(f, |nested| {
        format_expr_preserving(nested, key)?;
        nested.token(COLON);
        nested.space();
        fmt_pattern(nested, pattern, Parentheses::Preserve)
    })
}

fn fmt_pairs(f: &mut Formatter<'_>, node: NodeId, (keys, patterns, rest): PairsOf<'_>) -> Formatted {
    let mut joiner = CommaSeparated::new(f.ast.end(node));
    for (key, pattern) in keys.iter().zip(patterns) {
        let end = f.ast.end(*pattern);
        joiner.entry(f, end, |entry| fmt_pair(entry, *key, *pattern))?;
    }
    if let Some(rest) = rest {
        joiner.entry(f, rest.end, |entry| {
            entry.token(DOUBLE_STAR);
            entry.source_text(rest);
            Ok(())
        })?;
    }
    joiner.finish(f);
    Ok(())
}

type PairsOf<'ast> = (
    &'ast [NodeId],
    &'ast [NodeId],
    Option<crate::formatter::python::ast::Span>,
);

pub fn fmt_mapping(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::PatternMapping { keys, patterns, rest }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PATTERN));
    };
    let dangling = f.comments.dangling(node);
    if keys.is_empty() && rest.is_none() {
        return empty_parenthesized(f, ("{", "}"), dangling);
    }
    if rest.is_some() && !dangling.is_empty() {
        return Err(f.unsupported(node, DICT_COMMENT));
    }
    parenthesized(f, ("{", "}"), dangling, |nested| {
        fmt_pairs(nested, node, (keys, patterns, *rest))
    })
}
