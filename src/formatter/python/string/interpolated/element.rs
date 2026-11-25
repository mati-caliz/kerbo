use super::super::flags::StringFlags;
use super::super::normalize::{fstring_flags, normalize_string};
use super::quotes::elements_of;
use crate::formatter::document::{LineMode, remove_soft_line_breaks};
use crate::formatter::python::ast::{Conversion, DebugText, Expr, NodeId, Other};
use crate::formatter::python::builders::soft_block_indent;
use crate::formatter::python::context::{
    Formatted, Formatter, InterpolatedContext, InterpolatedState, NodeLevel,
};
use crate::formatter::python::expression::format_expr_preserving;
use crate::formatter::python::expression::left_most::left_most;
use crate::formatter::python::trivia::is_line_break;

const OPENING_BRACE: &str = "{";
const CLOSING_BRACE: &str = "}";
const COLON: &str = ":";
const CARRIAGE_RETURN: &str = "\r";
const CARRIAGE_RETURN_LINE_FEED: &str = "\r\n";
const LINE_FEED: &str = "\n";

#[derive(Debug, Clone, Copy)]
struct Replacement {
    expression: NodeId,
    conversion: Conversion,
    format_spec: Option<NodeId>,
    has_bracket_spacing: bool,
    is_multiline: bool,
    flags: StringFlags,
}

pub fn is_multiline_layout(f: &Formatter<'_>, part: NodeId) -> bool {
    elements_of(f, part).iter().any(|element| {
        matches!(f.ast.other(*element), Some(Other::Interpolation { .. }))
            && f.node_text(*element).contains(is_line_break)
    })
}

fn elements_are_multiline(f: &Formatter<'_>, elements: &[NodeId], is_triple_quoted: bool) -> bool {
    elements.iter().any(|element| match f.ast.other(*element) {
        Some(Other::InterpolatedLiteral) => is_triple_quoted && f.node_text(*element).contains(is_line_break),
        Some(Other::Interpolation {
            debug_text,
            format_spec,
            ..
        }) => {
            f.comments.contains_comments(*element)
                || format_spec
                    .is_some_and(|spec| elements_are_multiline(f, elements_of(f, spec), is_triple_quoted))
                || debug_text.is_some_and(|debug| {
                    f.text_of(debug.leading).contains(is_line_break)
                        || f.text_of(debug.trailing).contains(is_line_break)
                })
        }
        Some(_) | None => false,
    })
}

pub fn is_multiline_part(f: &Formatter<'_>, part: NodeId) -> bool {
    let flags = StringFlags::of(f.node_text(part));
    elements_are_multiline(f, elements_of(f, part), flags.is_triple_quoted)
}

pub fn fmt_fstring_part(f: &mut Formatter<'_>, part: NodeId) -> Formatted {
    let flags = fstring_flags(f, part);
    let context = InterpolatedContext {
        quote: flags.quote,
        is_triple_quoted: flags.is_triple_quoted,
        is_multiline: is_multiline_layout(f, part),
    };
    f.token(flags.prefix());
    f.token(flags.quotes());
    fmt_elements(f, elements_of(f, part), flags, context)?;
    f.token(flags.quotes());
    Ok(())
}

pub fn fmt_elements(
    f: &mut Formatter<'_>,
    elements: &[NodeId],
    flags: StringFlags,
    context: InterpolatedContext,
) -> Formatted {
    let saved = f.interpolated;
    f.interpolated = saved.enter_string(context);
    let result = elements
        .iter()
        .try_for_each(|element| fmt_element(f, *element, flags, context));
    f.interpolated = saved;
    result
}

fn fmt_element(
    f: &mut Formatter<'_>,
    element: NodeId,
    flags: StringFlags,
    context: InterpolatedContext,
) -> Formatted {
    match f.ast.other(element) {
        Some(Other::InterpolatedLiteral) => {
            let normalized = normalize_string(f.node_text(element), 0, flags, false);
            f.text(&normalized);
            Ok(())
        }
        Some(Other::Interpolation {
            expression,
            debug_text,
            conversion,
            format_spec,
        }) => {
            if f.comments.contains_comments(element) {
                return Err(f.unsupported(element, "comentario dentro de un f-string"));
            }
            let replacement = Replacement {
                expression: *expression,
                conversion: *conversion,
                format_spec: *format_spec,
                has_bracket_spacing: false,
                is_multiline: false,
                flags,
            };
            match debug_text {
                Some(debug) => {
                    fmt_debug(f, replacement, *debug);
                    Ok(())
                }
                None => fmt_interpolation(f, element, replacement, context),
            }
        }
        Some(_) | None => Err(f.unsupported(element, "elemento de f-string")),
    }
}

fn normalized_newlines(text: &str) -> String {
    text.replace(CARRIAGE_RETURN_LINE_FEED, LINE_FEED)
        .replace(CARRIAGE_RETURN, LINE_FEED)
}

fn write_normalized(f: &mut Formatter<'_>, text: &str) {
    if !text.is_empty() {
        f.text(&normalized_newlines(text));
    }
}

fn fmt_conversion(f: &mut Formatter<'_>, conversion: Conversion) {
    match conversion {
        Conversion::Str => f.token("!s"),
        Conversion::Ascii => f.token("!a"),
        Conversion::Repr => f.token("!r"),
        Conversion::None => {}
    }
}

fn fmt_debug(f: &mut Formatter<'_>, replacement: Replacement, debug: DebugText) {
    f.token(OPENING_BRACE);
    write_normalized(f, f.text_of(debug.leading));
    f.node_source(replacement.expression);
    write_normalized(f, f.text_of(debug.trailing));
    fmt_conversion(f, replacement.conversion);
    if let Some(spec) = replacement.format_spec {
        f.token(COLON);
        f.node_source(spec);
    }
    f.token(CLOSING_BRACE);
}

fn needs_bracket_spacing(f: &Formatter<'_>, expression: NodeId) -> bool {
    if matches!(
        f.ast.expr(expression),
        Some(Expr::Tuple {
            elts,
            parenthesized: false
        }) if elts.len() == 1
    ) {
        return false;
    }
    matches!(
        f.ast.expr(left_most(f, expression)),
        Some(Expr::Dict { .. } | Expr::DictComp { .. } | Expr::Set { .. } | Expr::SetComp { .. })
    )
}

fn fmt_bracket_spacing(f: &mut Formatter<'_>, replacement: Replacement) {
    if !replacement.has_bracket_spacing {
        return;
    }
    if replacement.is_multiline {
        f.line(LineMode::SoftOrSpace);
    } else {
        f.space();
    }
}

fn fmt_item(f: &mut Formatter<'_>, replacement: Replacement, context: InterpolatedContext) -> Formatted {
    let saved = f.interpolated;
    f.interpolated = match saved {
        InterpolatedState::Nested(_) => InterpolatedState::Nested(context),
        InterpolatedState::Inside(_) | InterpolatedState::Outside => InterpolatedState::Inside(context),
    };
    fmt_bracket_spacing(f, replacement);
    format_expr_preserving(f, replacement.expression)?;
    fmt_conversion(f, replacement.conversion);
    if let Some(spec) = replacement.format_spec {
        f.token(COLON);
        for element in elements_of(f, spec) {
            fmt_element(f, *element, replacement.flags, context)?;
        }
    }
    if replacement.conversion == Conversion::None && replacement.format_spec.is_none() {
        fmt_bracket_spacing(f, replacement);
    }
    f.interpolated = saved;
    Ok(())
}

fn has_line_break_before_spec(f: &Formatter<'_>, element: NodeId, format_spec: Option<NodeId>) -> bool {
    let end = format_spec.map_or_else(|| f.ast.end(element), |spec| f.ast.start(spec));
    f.source
        .get(f.ast.start(element)..end)
        .unwrap_or_default()
        .contains(is_line_break)
}

fn fmt_interpolation(
    f: &mut Formatter<'_>,
    element: NodeId,
    replacement: Replacement,
    outer: InterpolatedContext,
) -> Formatted {
    let is_multiline = outer.is_multiline
        && (outer.is_triple_quoted || has_line_break_before_spec(f, element, replacement.format_spec));
    let context = InterpolatedContext {
        is_multiline,
        ..outer
    };
    let replacement = Replacement {
        has_bracket_spacing: needs_bracket_spacing(f, replacement.expression),
        is_multiline,
        ..replacement
    };
    f.token(OPENING_BRACE);
    let saved_level = f.enter(NodeLevel::ParenthesizedExpression);
    if outer.is_multiline {
        f.start_group(None);
        if replacement.format_spec.is_none() {
            soft_block_indent(f, |nested| fmt_item(nested, replacement, context))?;
        } else {
            f.start_indent();
            f.line(LineMode::Soft);
            fmt_item(f, replacement, context)?;
            f.end_indent();
        }
        f.end_group();
    } else {
        let start = f.len();
        fmt_item(f, replacement, context)?;
        let written = f.take_since(start);
        for flat in remove_soft_line_breaks(&written) {
            f.push(flat);
        }
    }
    f.node_level = saved_level;
    f.token(CLOSING_BRACE);
    Ok(())
}
