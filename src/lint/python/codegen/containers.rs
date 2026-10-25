use tree_sitter::Node;

use super::{
    ASYNC, BIT_OR, COMMA, COMPREHENSION, COMPREHENSION_ELEMENT, COMPREHENSION_TARGET, FOR_IN_CLAUSE,
    IF_CLAUSE, KEY_FIELD, LEFT_FIELD, MAX, PAIR, RIGHT_FIELD, SUBSCRIPT, TUPLE, VALUE_FIELD, first, group,
    significant, unparse,
};
use crate::lint::python::ast::{COMMENT, text};

pub fn items(node: Node<'_>, source: &[u8]) -> String {
    significant(node)
        .into_iter()
        .map(|item| unparse(item, COMMA, source))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn tuple(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let elements = significant(node);
    if elements.is_empty() {
        return String::from("()");
    }
    let trailing = if elements.len() == 1 { "," } else { "" };
    group(level, TUPLE, format!("{}{trailing}", items(node, source)))
}

pub fn dictionary(node: Node<'_>, source: &[u8]) -> String {
    let entries: Vec<String> = significant(node)
        .into_iter()
        .map(|entry| {
            if entry.kind() == PAIR {
                let key = entry
                    .child_by_field_name(KEY_FIELD)
                    .map(|key| unparse(key, COMMA, source))
                    .unwrap_or_default();
                let value = entry
                    .child_by_field_name(VALUE_FIELD)
                    .map(|value| unparse(value, COMMA, source))
                    .unwrap_or_default();
                format!("{key}: {value}")
            } else {
                unparse(entry, COMMA, source)
            }
        })
        .collect();
    format!("{{{}}}", entries.join(", "))
}

pub fn comprehension(node: Node<'_>, source: &[u8]) -> String {
    let parts = significant(node);
    let mut rendered = String::new();
    for part in parts {
        match part.kind() {
            FOR_IN_CLAUSE => {
                let is_async = text(part, source).starts_with(ASYNC);
                rendered.push_str(if is_async { " async for " } else { " for " });
                if let Some(target) = part.child_by_field_name(LEFT_FIELD) {
                    rendered.push_str(&unparse(target, COMPREHENSION_TARGET, source));
                }
                rendered.push_str(" in ");
                if let Some(iterable) = part.child_by_field_name(RIGHT_FIELD) {
                    rendered.push_str(&unparse(iterable, COMPREHENSION, source));
                }
            }
            IF_CLAUSE => {
                rendered.push_str(" if ");
                rendered.push_str(&first(part, COMPREHENSION, source));
            }
            PAIR => {
                let key = part
                    .child_by_field_name(KEY_FIELD)
                    .map(|key| unparse(key, COMPREHENSION_ELEMENT, source))
                    .unwrap_or_default();
                let value = part
                    .child_by_field_name(VALUE_FIELD)
                    .map(|value| unparse(value, COMPREHENSION_ELEMENT, source))
                    .unwrap_or_default();
                rendered.push_str(&key);
                rendered.push_str(": ");
                rendered.push_str(&value);
            }
            _ => rendered.push_str(&unparse(part, COMPREHENSION_ELEMENT, source)),
        }
    }
    rendered
}

// `int | str` adentro de una anotación: es un `|` de ruff.
pub fn union_type(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let sides = significant(node);
    let rendered: Vec<String> = sides
        .iter()
        .enumerate()
        .map(|(index, side)| {
            let side_level = if index == 0 {
                BIT_OR
            } else {
                BIT_OR.saturating_add(1)
            };
            unparse(*side, side_level, source)
        })
        .collect();
    group(level, BIT_OR, rendered.join(" | "))
}

// `List[int]` adentro de una anotación: un subíndice de ruff.
pub fn generic_type(node: Node<'_>, source: &[u8]) -> String {
    let parts = significant(node);
    let Some((value, rest)) = parts.split_first() else {
        return text(node, source).to_owned();
    };
    let parameters: Vec<Node<'_>> = rest
        .iter()
        .flat_map(|parameters| significant(*parameters))
        .collect();
    let inner = match parameters.as_slice() {
        [only] => unparse(*only, SUBSCRIPT, source),
        _ => parameters
            .iter()
            .map(|parameter| unparse(*parameter, COMMA, source))
            .collect::<Vec<_>>()
            .join(", "),
    };
    format!("{}[{inner}]", unparse(*value, MAX, source))
}

pub fn member_type(node: Node<'_>, source: &[u8]) -> String {
    let parts = significant(node);
    match parts.as_slice() {
        [object, name] => format!("{}.{}", unparse(*object, MAX, source), text(*name, source)),
        _ => text(node, source).to_owned(),
    }
}

pub fn slice(node: Node<'_>, source: &[u8]) -> String {
    let mut rendered = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == ":" {
            rendered.push(':');
        } else if child.is_named() && child.kind() != COMMENT {
            rendered.push_str(&unparse(child, SUBSCRIPT, source));
        }
    }
    rendered
}
