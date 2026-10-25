use tree_sitter::Node;

use super::Report;
use super::ast::{
    COMMENT, CONCATENATED_STRING, FALSE, SET, TRUE, is_bytes, is_f_string, is_string, named, string_prefix,
    text,
};
use super::escapes::text_value;
use super::fix_edits::{commented_unsafe, remove_member};
use crate::rules::PythonRule;

const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const NONE_LITERAL: &str = "none";
const ELLIPSIS: &str = "ellipsis";
const DIGIT_SEPARATOR: char = '_';
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const RADIX_PREFIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];

#[derive(Debug, PartialEq, Eq)]
enum Hashable {
    Number(String),
    Text(String),
    Bytes(String),
    Constant(&'static str),
}

// B033: un elemento repetido en un conjunto escrito. Como el `HashableExpr` de ruff, compara los
// literales por valor: `1`, `1.0` y `True` son el mismo elemento.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == SET)
        .flat_map(|set| duplicates(*set, source))
        .collect()
}

fn duplicates(set: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut seen = Vec::new();
    let mut found = Vec::new();
    for element in named(set).into_iter().filter(|element| element.kind() != COMMENT) {
        let Some(value) = hashable(element, source) else {
            continue;
        };
        if seen.contains(&value) {
            let fix = commented_unsafe(remove_member(element, set, source), set, source);
            found.push(Report::at(PythonRule::B033, element, "").with_fix(Some(fix)));
        } else {
            seen.push(value);
        }
    }
    found
}

fn hashable(element: Node<'_>, source: &[u8]) -> Option<Hashable> {
    match element.kind() {
        TRUE => Some(Hashable::Number(1.to_string())),
        FALSE => Some(Hashable::Number(0.to_string())),
        NONE_LITERAL => Some(Hashable::Constant(NONE_LITERAL)),
        ELLIPSIS => Some(Hashable::Constant(ELLIPSIS)),
        INTEGER | FLOAT => Some(Hashable::Number(number_value(text(element, source)))),
        _ if is_string(element) => string_value(element, source),
        _ => None,
    }
}

fn string_value(element: Node<'_>, source: &[u8]) -> Option<Hashable> {
    let parts = if element.kind() == CONCATENATED_STRING {
        named(element)
    } else {
        vec![element]
    };
    let is_template = |part: &Node<'_>| string_prefix(*part, source).contains(TEMPLATE_PREFIXES);
    if parts
        .iter()
        .any(|part| is_f_string(*part, source) || is_template(part))
    {
        return None;
    }
    if parts.iter().all(|part| is_bytes(*part, source)) {
        let raw: String = parts
            .iter()
            .map(|part| {
                let literal = text(*part, source);
                literal
                    .get(string_prefix(*part, source).len()..)
                    .unwrap_or(literal)
                    .trim_matches(['"', '\''])
                    .to_owned()
            })
            .collect();
        return Some(Hashable::Bytes(raw));
    }
    text_value(element, source).map(Hashable::Text)
}

fn number_value(literal: &str) -> String {
    let digits: String = literal
        .chars()
        .filter(|digit| *digit != DIGIT_SEPARATOR)
        .collect();
    if digits.ends_with(IMAGINARY_SUFFIXES) {
        return digits;
    }
    let lowered = digits.to_ascii_lowercase();
    for (prefix, radix) in RADIX_PREFIXES {
        if let Some(rest) = lowered.strip_prefix(prefix) {
            return u128::from_str_radix(rest, radix).map_or(digits, |value| value.to_string());
        }
    }
    if let Ok(value) = lowered.parse::<u128>() {
        return value.to_string();
    }
    match lowered.parse::<f64>() {
        Ok(value) if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e18 => {
            format!("{value:.0}")
        }
        Ok(value) => value.to_string(),
        Err(_) => digits,
    }
}
