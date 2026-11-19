use tree_sitter::Node;

use super::collections::generator;
use super::expressions::{element_or_tuple, expression};
use super::{Converter, Outcome, field, named, only_named, parts, span};
use crate::formatter::python::ast::{Expr, NodeId, Other, Span};

const GENERATOR_EXPRESSION: &str = "generator_expression";
const KEYWORD_ARGUMENT: &str = "keyword_argument";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const COLON: &str = ":";

pub fn convert<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "call" => Expr::Call {
            func: expression(converter, field(node, "function")?)?,
            arguments: arguments(converter, field(node, "arguments")?)?,
        },
        "subscript" => subscript(converter, node)?,
        "slice" => slice(converter, node)?,
        _ => return Err(node),
    };
    Ok(converter.expression(node, added))
}

// El tramo de adentro de los paréntesis: del primer hijo con nombre al último.
fn inner_span(node: Node<'_>) -> Span {
    let children = named(node);
    match (children.first(), children.last()) {
        (Some(first), Some(last)) => Span::new(first.start_byte(), last.end_byte()),
        _ => span(node),
    }
}

pub fn arguments<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut args = Vec::new();
    let mut keywords = Vec::new();
    if node.kind() == GENERATOR_EXPRESSION {
        args.push(generator(converter, node, inner_span(node))?);
    } else {
        for argument in named(node) {
            match argument.kind() {
                KEYWORD_ARGUMENT => {
                    let keyword = Other::Keyword {
                        arg: Some(span(field(argument, "name")?)),
                        value: expression(converter, field(argument, "value")?)?,
                    };
                    keywords.push(converter.other(argument, keyword));
                }
                DICTIONARY_SPLAT => {
                    let keyword = Other::Keyword {
                        arg: None,
                        value: expression(converter, only_named(argument)?)?,
                    };
                    keywords.push(converter.other(argument, keyword));
                }
                _ => args.push(expression(converter, argument)?),
            }
        }
    }
    Ok(converter.other(node, Other::Arguments { args, keywords }))
}

fn subscript<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let mut cursor = node.walk();
    let indexes: Vec<Node<'tree>> = node.children_by_field_name("subscript", &mut cursor).collect();
    Ok(Expr::Subscript {
        value: expression(converter, field(node, "value")?)?,
        slice: element_or_tuple(converter, node, &indexes)?,
    })
}

// `lower:upper:step`: cada `:` pasa a la parte siguiente, y cualquiera puede faltar.
fn slice<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let mut bounds: [Option<NodeId>; 3] = [None, None, None];
    let mut position = 0_usize;
    for part in parts(node) {
        if part.kind() == COLON {
            position = position.saturating_add(1);
            continue;
        }
        let bound = bounds.get_mut(position).ok_or(part)?;
        *bound = Some(expression(converter, part)?);
    }
    let [lower, upper, step] = bounds;
    Ok(Expr::Slice { lower, upper, step })
}
