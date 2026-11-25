use tree_sitter::Node;

use super::interpolated::fstring_part;
use super::{Converter, Outcome, named};
use crate::formatter::python::ast::{Expr, NodeId, Other};

const STRING_START: &str = "string_start";
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const QUOTES: [char; 2] = ['"', '\''];
const BYTES_PREFIXES: [char; 2] = ['b', 'B'];
const FORMAT_PREFIXES: [char; 2] = ['f', 'F'];

// Una cadena o varias pegadas (concatenación implícita). Cada parte es un nodo, como en ruff.
pub fn convert<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let literals = if node.kind() == "string" {
        vec![node]
    } else {
        named(node)
    };
    let mut parts = Vec::with_capacity(literals.len());
    let mut bytes_parts = 0_usize;
    for literal in &literals {
        let (part, is_bytes) = part(converter, *literal)?;
        parts.push(part);
        bytes_parts = bytes_parts.saturating_add(usize::from(is_bytes));
    }
    let added = if bytes_parts == 0 {
        Expr::StringLiteral { parts }
    } else if bytes_parts == parts.len() {
        Expr::BytesLiteral { parts }
    } else {
        return Err(node);
    };
    Ok(converter.expression(node, added))
}

fn part<'tree>(converter: &mut Converter<'_>, literal: Node<'tree>) -> Outcome<'tree, (NodeId, bool)> {
    let children = named(literal);
    let start = children
        .iter()
        .find(|child| child.kind() == STRING_START)
        .ok_or(literal)?;
    let prefix = converter.text(*start).trim_end_matches(QUOTES);
    if prefix.contains(FORMAT_PREFIXES) || prefix.contains(TEMPLATE_PREFIXES) {
        return Ok((fstring_part(converter, literal)?, false));
    }
    let is_bytes = prefix.contains(BYTES_PREFIXES);
    let added = if is_bytes {
        Other::BytesPart
    } else {
        Other::StringPart
    };
    Ok((converter.other(literal, added), is_bytes))
}
