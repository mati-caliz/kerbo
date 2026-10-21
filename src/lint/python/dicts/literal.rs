use tree_sitter::Node;

use super::super::ast::{
    CONCATENATED_STRING, FALSE, IDENTIFIER, NONE, STRING, TRUE, TUPLE, is_bytes, is_f_string, named, text,
    unparenthesized,
};
use super::super::escapes::part_value;

const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const ELLIPSIS: &str = "ellipsis";
const UNARY: &str = "unary_operator";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const DIGIT_SEPARATOR: char = '_';
const IMAGINARY: [char; 2] = ['j', 'J'];
const RADIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];
const DECIMAL: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Text(String),
    Bytes(String),
    Integer(i128),
    Float(u64),
    Complex(String),
    Singleton(&'static str),
    Name(String),
    Tuple(Vec<Self>),
    Unary(String, Box<Self>),
    Raw(String),
}

pub fn key(node: Node<'_>, source: &[u8]) -> Option<Key> {
    let node = unparenthesized(node);
    match node.kind() {
        IDENTIFIER => Some(Key::Name(text(node, source).to_owned())),
        TUPLE => Some(Key::Tuple(
            named(node)
                .into_iter()
                .map(|element| element_key(element, source))
                .collect(),
        )),
        UNARY => unary(node, source),
        _ => literal(node, source),
    }
}

fn element_key(node: Node<'_>, source: &[u8]) -> Key {
    key(node, source).unwrap_or_else(|| Key::Raw(text(node, source).to_owned()))
}

fn unary(node: Node<'_>, source: &[u8]) -> Option<Key> {
    let operator = node.child_by_field_name(OPERATOR_FIELD)?;
    let argument = node.child_by_field_name(ARGUMENT_FIELD)?;
    Some(Key::Unary(
        text(operator, source).to_owned(),
        Box::new(element_key(argument, source)),
    ))
}

fn literal(node: Node<'_>, source: &[u8]) -> Option<Key> {
    match node.kind() {
        NONE | ELLIPSIS => Some(Key::Singleton(if node.kind() == NONE { NONE } else { ELLIPSIS })),
        TRUE => Some(Key::Integer(1)),
        FALSE => Some(Key::Integer(0)),
        INTEGER | FLOAT => Some(number(text(node, source))),
        STRING | CONCATENATED_STRING => Some(string(node, source)),
        _ => None,
    }
}

fn number(literal: &str) -> Key {
    let digits: String = literal
        .chars()
        .filter(|digit| *digit != DIGIT_SEPARATOR)
        .collect();
    let lowered = digits.to_lowercase();
    if lowered.ends_with(IMAGINARY) {
        return Key::Complex(lowered);
    }
    let radix = RADIXES
        .iter()
        .find_map(|(prefix, radix)| lowered.strip_prefix(prefix).map(|rest| (rest, *radix)));
    let (body, radix) = radix.unwrap_or((lowered.as_str(), DECIMAL));
    if let Ok(integer) = i128::from_str_radix(body, radix) {
        return Key::Integer(integer);
    }
    lowered.parse::<f64>().map_or_else(
        |_| Key::Raw(lowered.clone()),
        |float| integral(float).map_or_else(|| Key::Float(float.to_bits()), Key::Integer),
    )
}

fn integral(float: f64) -> Option<i128> {
    let truncated = float.trunc();
    let text = format!("{truncated:.0}");
    (truncated.to_bits() == float.to_bits())
        .then(|| text.parse().ok())
        .flatten()
}

fn string(node: Node<'_>, source: &[u8]) -> Key {
    let parts = if node.kind() == STRING {
        vec![node]
    } else {
        named(node)
    };
    if parts.iter().any(|part| is_f_string(*part, source)) {
        return Key::Raw(text(node, source).to_owned());
    }
    let value: String = parts.iter().map(|part| part_value(*part, source)).collect();
    if parts.iter().any(|part| is_bytes(*part, source)) {
        Key::Bytes(value)
    } else {
        Key::Text(value)
    }
}
