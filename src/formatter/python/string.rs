mod docstring;
mod flags;
pub mod implicit;
mod indentation;
mod interpolated;
mod normalize;
mod quotes;

use super::ast::{Expr, NodeId};
use super::builders::parenthesize_if_expands;
use super::context::{Formatted, Formatter};
use super::expression::format_node_with;
use super::parentheses::in_parentheses_only_group;
use super::trivia::is_line_break;
use flags::StringFlags;
use implicit::{ImplicitLayout, fmt_expanded, fmt_flat, fmt_implicit_concatenated_string, merged_flags};
use interpolated::quotes::is_fstring_part;
use interpolated::{fmt_fstring_part, is_multiline_layout, is_multiline_part};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    Default,
    Docstring,
}

pub fn parts_of<'source>(f: &Formatter<'source>, expression: NodeId) -> &'source [NodeId] {
    match f.ast.expr(expression) {
        Some(Expr::StringLiteral { parts } | Expr::BytesLiteral { parts }) => parts,
        Some(_) | None => &[],
    }
}

fn part_is_multiline(f: &Formatter<'_>, part: NodeId) -> bool {
    if is_fstring_part(f, part) {
        return is_multiline_part(f, part);
    }
    let text = f.node_text(part);
    StringFlags::of(text).is_triple_quoted && text.contains(is_line_break)
}

// Una cadena de comillas triples que ocupa más de una línea.
pub fn is_multiline(f: &Formatter<'_>, expression: NodeId) -> bool {
    parts_of(f, expression)
        .iter()
        .any(|part| part_is_multiline(f, *part))
}

pub fn is_implicit_concatenated(f: &Formatter<'_>, expression: NodeId) -> bool {
    parts_of(f, expression).len() > 1
}

pub fn is_single_multiline_triple_quoted(f: &Formatter<'_>, expression: NodeId) -> bool {
    matches!(parts_of(f, expression), [part] if part_is_multiline(f, *part))
}

// Una sola parte que es un f-string con una expresión en más de una línea.
pub fn has_multiline_replacement_field(f: &Formatter<'_>, expression: NodeId) -> bool {
    matches!(parts_of(f, expression), [part] if is_fstring_part(f, *part) && is_multiline_layout(f, *part))
}

pub fn is_plain_string(f: &Formatter<'_>, expression: NodeId) -> bool {
    parts_of(f, expression)
        .iter()
        .all(|part| !is_fstring_part(f, *part))
}

fn fmt_part(f: &mut Formatter<'_>, part: NodeId, kind: StringKind) -> Formatted {
    format_node_with(f, part, |f| {
        if is_fstring_part(f, part) {
            return fmt_fstring_part(f, part);
        }
        let normalized = normalize::normalize(f, part);
        if kind == StringKind::Docstring {
            docstring::fmt(f, &normalized);
        } else {
            normalize::fmt_normalized(f, &normalized);
        }
        Ok(())
    })
}

// Una cadena o bytes: una sola parte se normaliza; varias pegadas se unen en una si entran en la
// línea, y si no, va una por línea.
pub fn fmt_string(f: &mut Formatter<'_>, expression: NodeId, kind: StringKind) -> Formatted {
    let parts = parts_of(f, expression);
    if let [part] = parts {
        return fmt_part(f, *part, kind);
    }
    let is_docstring = kind == StringKind::Docstring;
    if !f.node_level.is_parenthesized() {
        if let Some(flags) = merged_flags(f, expression) {
            fmt_flat(f, expression, flags, is_docstring)?;
            return Ok(());
        }
        if is_docstring {
            return parenthesize_if_expands(f, true, |f| {
                fmt_expanded(f, expression, ImplicitLayout::Multipart)
            });
        }
    }
    in_parentheses_only_group(f, |f| fmt_implicit_concatenated_string(f, expression))
}

pub fn fmt_string_expression(f: &mut Formatter<'_>, expression: NodeId) -> Formatted {
    fmt_string(f, expression, StringKind::Default)
}

pub fn fmt_plain_part(f: &mut Formatter<'_>, part: NodeId) -> Formatted {
    fmt_part(f, part, StringKind::Default)
}
