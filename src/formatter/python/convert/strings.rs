use tree_sitter::Node;

use super::{Converter, Outcome, named};
use crate::formatter::python::ast::{Expr, NodeId, Other};

const STRING_START: &str = "string_start";
const INTERPOLATION: &str = "interpolation";
const QUOTES: [char; 2] = ['"', '\''];
const BYTES_PREFIXES: [char; 2] = ['b', 'B'];
const INTERPOLATED_PREFIXES: [char; 4] = ['f', 'F', 't', 'T'];

// Una cadena o varias pegadas (concatenación implícita). Cada parte es un nodo, como en ruff. Las
// f-strings y las t-strings todavía no se formatean.
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
    let is_interpolated =
        prefix.contains(INTERPOLATED_PREFIXES) || children.iter().any(|child| child.kind() == INTERPOLATION);
    if is_interpolated {
        return Err(literal);
    }
    let is_bytes = prefix.contains(BYTES_PREFIXES);
    let added = if is_bytes {
        Other::BytesPart
    } else {
        Other::StringPart
    };
    Ok((converter.other(literal, added), is_bytes))
}
