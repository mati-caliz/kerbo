use tree_sitter::Node;

use super::expressions::expression_or_tuple;
use super::{Converter, Outcome, field, named, token};
use crate::formatter::python::ast::{Conversion, Data, DebugText, NodeId, Other, Span};

const STRING_START: &str = "string_start";
const STRING_END: &str = "string_end";
const INTERPOLATION: &str = "interpolation";
const FORMAT_EXPRESSION: &str = "format_expression";
const NAMED_EXPRESSION: &str = "named_expression";
const EQUALS: &str = "=";
const OPENING_BRACE: &str = "{";
const CONVERSION_FIELD: &str = "type_conversion";
const FORMAT_SPEC_FIELD: &str = "format_specifier";
const COLON_LENGTH: usize = 1;
const CLOSING_BRACE_LENGTH: usize = 1;

fn add_other(converter: &mut Converter<'_>, span: Span, other: Other) -> NodeId {
    converter.ast.add(span, Data::Other(other))
}

fn conversion_of(converter: &Converter<'_>, node: Option<Node<'_>>) -> Conversion {
    match node.map(|conversion| converter.text(conversion)) {
        Some("!s") => Conversion::Str,
        Some("!a") => Conversion::Ascii,
        Some("!r") => Conversion::Repr,
        _ => Conversion::None,
    }
}

fn debug_text_of(node: Node<'_>, expression_span: Span) -> Option<DebugText> {
    token(node, EQUALS)?;
    let opening = token(node, OPENING_BRACE)?;
    let after_debug = [CONVERSION_FIELD, FORMAT_SPEC_FIELD]
        .iter()
        .filter_map(|name| node.child_by_field_name(name))
        .map(|following| following.start_byte())
        .min()
        .unwrap_or_else(|| node.end_byte().saturating_sub(CLOSING_BRACE_LENGTH));
    Some(DebugText {
        leading: Span::new(opening.end_byte(), expression_span.start),
        trailing: Span::new(expression_span.end, after_debug),
    })
}

fn interpolation<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let expression_node = field(node, "expression")?;
    if expression_node.kind() == NAMED_EXPRESSION {
        return Err(node);
    }
    let expression = expression_or_tuple(converter, expression_node)?;
    let expression_span = converter.ast.span(expression);
    let debug_text = debug_text_of(node, expression_span);
    let conversion = conversion_of(converter, node.child_by_field_name(CONVERSION_FIELD));
    let format_spec = match node.child_by_field_name(FORMAT_SPEC_FIELD) {
        Some(specifier) => Some(format_spec(converter, specifier)?),
        None => None,
    };
    Ok(add_other(
        converter,
        Span::new(node.start_byte(), node.end_byte()),
        Other::Interpolation {
            expression,
            debug_text,
            conversion,
            format_spec,
        },
    ))
}

fn format_spec<'tree>(converter: &mut Converter<'_>, specifier: Node<'tree>) -> Outcome<'tree> {
    let content = Span::new(
        specifier.start_byte().saturating_add(COLON_LENGTH),
        specifier.end_byte(),
    );
    let nested: Vec<Node<'tree>> = named(specifier)
        .into_iter()
        .filter(|child| child.kind() == FORMAT_EXPRESSION)
        .collect();
    let elements = elements_between(converter, content, &nested)?;
    Ok(add_other(converter, content, Other::FormatSpec { elements }))
}

fn elements_between<'tree>(
    converter: &mut Converter<'_>,
    content: Span,
    interpolations: &[Node<'tree>],
) -> Outcome<'tree, Vec<NodeId>> {
    let mut elements = Vec::new();
    let mut cursor = content.start;
    for node in interpolations {
        push_literal(converter, &mut elements, Span::new(cursor, node.start_byte()));
        elements.push(interpolation(converter, *node)?);
        cursor = node.end_byte();
    }
    push_literal(converter, &mut elements, Span::new(cursor, content.end));
    Ok(elements)
}

fn push_literal(converter: &mut Converter<'_>, elements: &mut Vec<NodeId>, span: Span) {
    if span.start < span.end {
        elements.push(add_other(converter, span, Other::InterpolatedLiteral));
    }
}

pub fn fstring_part<'tree>(converter: &mut Converter<'_>, literal: Node<'tree>) -> Outcome<'tree> {
    let children = named(literal);
    let start = children
        .iter()
        .find(|child| child.kind() == STRING_START)
        .ok_or(literal)?;
    let end = children
        .iter()
        .find(|child| child.kind() == STRING_END)
        .ok_or(literal)?;
    let nested: Vec<Node<'tree>> = children
        .iter()
        .copied()
        .filter(|child| child.kind() == INTERPOLATION)
        .collect();
    let content = Span::new(start.end_byte(), end.start_byte());
    let elements = elements_between(converter, content, &nested)?;
    Ok(converter.other(literal, Other::FStringPart { elements }))
}
