use super::docstring::needs_chaperone_space;
use super::flags::StringFlags;
use super::normalize::{normalize_string, preferred_quote};
use super::quotes::QuoteMetadata;
use super::{fmt_plain_part, is_multiline, parts_of};
use crate::formatter::document::PrintMode;
use crate::formatter::python::ast::{Expr, NodeId, Span};
use crate::formatter::python::comments::format::{leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter, Quote};
use crate::formatter::python::parentheses::in_parentheses_only_soft_line_break_or_space;
use crate::formatter::python::trivia::is_line_break;

const BYTES_FLAGS_SOURCE: &str = "b\"\"";
const STRING_FLAGS_SOURCE: &str = "\"\"";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitLayout {
    MaybeFlat,
    Multipart,
}

fn content_of<'source>(f: &Formatter<'source>, part: NodeId) -> (&'source str, StringFlags) {
    let flags = StringFlags::of(f.node_text(part));
    (f.text_of(flags.content_span(f.ast.span(part))), flags)
}

fn merged_quote(f: &Formatter<'_>, parts: &[NodeId], preferred: Quote) -> Option<Quote> {
    let mut merged: Option<QuoteMetadata> = None;
    for part in parts {
        let (content, flags) = content_of(f, *part);
        let metadata = QuoteMetadata::from_str(content, flags, preferred);
        merged = Some(match merged {
            Some(previous) => metadata.merge(previous)?,
            None => metadata,
        });
    }
    merged.map(|metadata| metadata.choose(preferred))
}

// Las comillas y el prefijo de la cadena única en que se pueden unir las partes, si se puede: no
// con comillas triples, crudas, comentarios entre las partes o saltos de línea adentro.
pub fn merged_flags(f: &Formatter<'_>, string: NodeId) -> Option<StringFlags> {
    let parts = parts_of(f, string);
    let first_part = parts.first()?;
    if parts.len() < 2 || is_multiline(f, string) {
        return None;
    }
    let comments = f.comments;
    let can_join = parts.iter().all(|part| {
        let flags = StringFlags::of(f.node_text(*part));
        !flags.is_triple_quoted
            && !flags.is_raw()
            && !comments.has_leading(*part)
            && !comments.has_trailing(*part)
    });
    if !can_join {
        return None;
    }
    let preferred = preferred_quote(f, StringFlags::of(f.node_text(*first_part)));
    let quote = merged_quote(f, parts, preferred)?;
    let is_bytes = matches!(f.ast.expr(string), Some(Expr::BytesLiteral { .. }));
    let prefix_source = if is_bytes {
        BYTES_FLAGS_SOURCE
    } else {
        STRING_FLAGS_SOURCE
    };
    Some(StringFlags::of(prefix_source).with_quote(quote))
}

fn fmt_literal_content(
    f: &mut Formatter<'_>,
    content: &str,
    flags: StringFlags,
    (trim_start, trim_end): (bool, bool),
) {
    let normalized = normalize_string(content, 0, flags);
    let mut text: &str = &normalized;
    if trim_start {
        text = text.trim_start();
    }
    if trim_end {
        text = text.trim_end();
    }
    if text.is_empty() {
        return;
    }
    f.text(text);
    if trim_end && needs_chaperone_space(flags, text) {
        f.space();
    }
}

// Las partes unidas en una sola cadena. En un docstring se recortan los espacios de las puntas.
pub fn fmt_flat(f: &mut Formatter<'_>, string: NodeId, flags: StringFlags, is_docstring: bool) {
    f.token(flags.prefix());
    f.token(flags.quotes());
    let mut parts = parts_of(f, string);
    if is_docstring {
        while let Some((last, before)) = parts.split_last() {
            if !content_of(f, *last).0.trim().is_empty() {
                break;
            }
            parts = before;
        }
    }
    let mut first_non_empty = is_docstring;
    for (index, part) in parts.iter().enumerate() {
        let content = content_of(f, *part).0;
        let is_last = index.saturating_add(1) == parts.len();
        fmt_literal_content(f, content, flags, (first_non_empty, is_docstring && is_last));
        if first_non_empty {
            first_non_empty = content.trim_start().is_empty();
        }
    }
    f.token(flags.quotes());
}

fn has_line_break_between_parts(f: &Formatter<'_>, parts: &[NodeId]) -> bool {
    parts.windows(2).any(|pair| match pair {
        [before, after] => f
            .text_of(Span::new(f.ast.end(*before), f.ast.start(*after)))
            .contains(is_line_break),
        _ => false,
    })
}

// Una parte por línea. Si en el fuente ya estaban en líneas distintas, quedan así.
pub fn fmt_expanded(f: &mut Formatter<'_>, string: NodeId, layout: ImplicitLayout) -> Formatted {
    let parts = parts_of(f, string);
    if layout == ImplicitLayout::Multipart && has_line_break_between_parts(f, parts) {
        f.expand_parent();
    }
    let comments = f.comments;
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            in_parentheses_only_soft_line_break_or_space(f);
        }
        leading_comments(f, comments.leading(*part));
        fmt_plain_part(f, *part)?;
        trailing_comments(f, comments.trailing(*part));
    }
    Ok(())
}

pub fn fmt_implicit_concatenated_string(f: &mut Formatter<'_>, string: NodeId) -> Formatted {
    let Some(flags) = merged_flags(f, string) else {
        return fmt_expanded(f, string, ImplicitLayout::Multipart);
    };
    f.start_conditional_content(PrintMode::Flat, None);
    fmt_flat(f, string, flags, false);
    f.end_conditional_content();
    f.start_conditional_content(PrintMode::Expanded, None);
    fmt_expanded(f, string, ImplicitLayout::MaybeFlat)?;
    f.end_conditional_content();
    Ok(())
}
