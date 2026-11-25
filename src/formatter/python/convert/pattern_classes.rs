use tree_sitter::Node;

use super::patterns::{add_other, case_patterns, dotted_name, pattern, value_expression, value_pattern};
use super::{Converter, Outcome, named, only_named, parts, span};
use crate::formatter::python::ast::{Other, Span};

const CASE_PATTERN: &str = "case_pattern";
const KEYWORD_PATTERN: &str = "keyword_pattern";
const SPLAT_PATTERN: &str = "splat_pattern";
const COMMA: &str = ",";

pub(super) fn mapping<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut keys = Vec::new();
    let mut patterns = Vec::new();
    let mut rest = None;
    let mut pending: Vec<Node<'tree>> = Vec::new();
    for child in parts(node) {
        match child.kind() {
            SPLAT_PATTERN => {
                let identifier = named(child).first().copied().ok_or(child)?;
                rest = Some(span(identifier));
            }
            CASE_PATTERN => {
                keys.push(value_expression(converter, node, &std::mem::take(&mut pending))?);
                patterns.push(pattern(converter, child)?);
            }
            "{" | "}" | COMMA | ":" if !child.is_named() => {}
            _ => pending.push(child),
        }
    }
    Ok(add_other(
        converter,
        span(node),
        Other::PatternMapping { keys, patterns, rest },
    ))
}

pub(super) fn class<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = parts(node);
    let name = children.first().copied().ok_or(node)?;
    let opening = children
        .iter()
        .find(|child| !child.is_named() && converter.text(**child) == "(")
        .ok_or(node)?;
    let cls = dotted_name(converter, name)?;
    let mut positional = Vec::new();
    let mut keywords = Vec::new();
    for argument in case_patterns(node) {
        let inner = only_named(argument)?;
        if inner.kind() == KEYWORD_PATTERN {
            keywords.push(keyword(converter, inner)?);
        } else {
            positional.push(pattern(converter, argument)?);
        }
    }
    let arguments = add_other(
        converter,
        Span::new(opening.start_byte(), node.end_byte()),
        Other::PatternArguments {
            patterns: positional,
            keywords,
        },
    );
    Ok(add_other(
        converter,
        span(node),
        Other::PatternClass { cls, arguments },
    ))
}

fn keyword<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = parts(node);
    let [attr, _equals, value @ ..] = children.as_slice() else {
        return Err(node);
    };
    let value = if let [single] = value {
        pattern(converter, *single)?
    } else {
        value_pattern(converter, node, value)?
    };
    Ok(add_other(
        converter,
        span(node),
        Other::PatternKeyword {
            attr: span(*attr),
            pattern: value,
        },
    ))
}
