use super::super::flags::StringFlags;
use super::super::quotes::QuoteMetadata;
use crate::formatter::python::ast::{NodeId, Other};
use crate::formatter::python::context::{Formatter, Quote};

const BACKSLASH: char = '\\';
const TRIPLE_QUOTE_LENGTH: usize = 3;
const ESCAPE_PAIR: usize = 2;

pub fn elements_of<'source>(f: &Formatter<'source>, node: NodeId) -> &'source [NodeId] {
    match f.ast.other(node) {
        Some(Other::FStringPart { elements } | Other::FormatSpec { elements }) => elements,
        Some(_) | None => &[],
    }
}

pub fn is_fstring_part(f: &Formatter<'_>, node: NodeId) -> bool {
    matches!(f.ast.other(node), Some(Other::FStringPart { .. }))
}

fn merge_elements(
    f: &Formatter<'_>,
    elements: &[NodeId],
    flags: StringFlags,
    (preferred, merged): (Quote, QuoteMetadata),
) -> QuoteMetadata {
    let mut merged = merged;
    for element in elements {
        match f.ast.other(*element) {
            Some(Other::InterpolatedLiteral) => {
                let literal = QuoteMetadata::from_str(f.node_text(*element), flags, preferred);
                merged = merged.merge(literal).unwrap_or(merged);
            }
            Some(Other::Interpolation {
                debug_text: None,
                format_spec: Some(spec),
                ..
            }) => {
                merged = merge_elements(f, elements_of(f, *spec), flags, (preferred, merged));
            }
            Some(_) | None => {}
        }
    }
    merged
}

pub fn quote_metadata(f: &Formatter<'_>, part: NodeId, preferred: Quote) -> QuoteMetadata {
    let flags = StringFlags::of(f.node_text(part));
    let start = QuoteMetadata::from_str("", flags, preferred);
    merge_elements(f, elements_of(f, part), flags, (preferred, start))
}

fn contains_opposite_quote(content: &str, flags: StringFlags) -> bool {
    let opposite = flags.quote.opposite().as_char();
    if flags.is_triple_quoted {
        return content.contains(&opposite.to_string().repeat(TRIPLE_QUOTE_LENGTH));
    }
    let mut rest = content;
    while let Some(index) = rest.find(opposite) {
        if flags.is_raw() {
            return true;
        }
        let before = rest.get(..index).unwrap_or_default();
        let backslashes = before
            .chars()
            .rev()
            .take_while(|character| *character == BACKSLASH)
            .count();
        if backslashes % ESCAPE_PAIR == 0 {
            return true;
        }
        rest = rest
            .get(index.saturating_add(opposite.len_utf8())..)
            .unwrap_or_default();
    }
    false
}

fn has_quoted_debug_expression(f: &Formatter<'_>, part: NodeId, flags: StringFlags) -> bool {
    elements_of(f, part).iter().any(|element| {
        matches!(
            f.ast.other(*element),
            Some(Other::Interpolation {
                debug_text: Some(_),
                ..
            })
        ) && contains_opposite_quote(f.node_text(*element), flags)
    })
}

fn spec_has_opposite_quote(
    f: &Formatter<'_>,
    elements: &[NodeId],
    flags: StringFlags,
    in_debug: bool,
) -> bool {
    elements.iter().any(|element| match f.ast.other(*element) {
        Some(Other::InterpolatedLiteral) => in_debug && contains_opposite_quote(f.node_text(*element), flags),
        Some(Other::Interpolation {
            debug_text,
            format_spec: Some(spec),
            ..
        }) => spec_has_opposite_quote(f, elements_of(f, *spec), flags, in_debug || debug_text.is_some()),
        Some(_) | None => false,
    })
}

fn has_quoted_format_spec_and_debug(f: &Formatter<'_>, part: NodeId, flags: StringFlags) -> bool {
    elements_of(f, part).iter().any(|element| {
        matches!(
            f.ast.other(*element),
            Some(Other::Interpolation {
                debug_text,
                format_spec: Some(spec),
                ..
            }) if spec_has_opposite_quote(f, elements_of(f, *spec), flags, debug_text.is_some())
        )
    })
}

fn collect_expressions(f: &Formatter<'_>, elements: &[NodeId], found: &mut Vec<NodeId>) {
    for element in elements {
        if let Some(Other::Interpolation {
            expression,
            format_spec,
            ..
        }) = f.ast.other(*element)
        {
            found.push(*expression);
            if let Some(spec) = format_spec {
                collect_expressions(f, elements_of(f, *spec), found);
            }
        }
    }
}

fn is_triple_quoted_with_quote(f: &Formatter<'_>, node: NodeId) -> bool {
    let flags = StringFlags::of(f.node_text(node));
    if !flags.is_triple_quoted {
        return false;
    }
    let quote = flags.quote.as_char();
    if is_fstring_part(f, node) {
        return elements_of(f, node).iter().any(|element| {
            matches!(f.ast.other(*element), Some(Other::InterpolatedLiteral))
                && f.node_text(*element).contains(quote)
        });
    }
    f.text_of(flags.content_span(f.ast.span(node))).contains(quote)
}

fn has_triple_quoted_literal_with_quotes(f: &Formatter<'_>, part: NodeId) -> bool {
    let mut pending = Vec::new();
    collect_expressions(f, elements_of(f, part), &mut pending);
    while let Some(node) = pending.pop() {
        match f.ast.other(node) {
            Some(Other::StringPart | Other::BytesPart | Other::FStringPart { .. }) => {
                if is_triple_quoted_with_quote(f, node) {
                    return true;
                }
            }
            Some(_) | None => pending.extend(f.ast.children(node)),
        }
    }
    false
}

pub fn must_preserve_quotes(f: &Formatter<'_>, part: NodeId) -> bool {
    let flags = StringFlags::of(f.node_text(part));
    has_quoted_debug_expression(f, part, flags)
        || has_triple_quoted_literal_with_quotes(f, part)
        || has_quoted_format_spec_and_debug(f, part, flags)
}

pub fn must_preserve_quotes_when_joined(f: &Formatter<'_>, part: NodeId) -> bool {
    let flags = StringFlags::of(f.node_text(part));
    has_quoted_debug_expression(f, part, flags) || has_triple_quoted_literal_with_quotes(f, part)
}
