use tree_sitter::Node;

use super::percent::{Quantity, Specifier, specifiers};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, CALL, CONCATENATED_STRING, IDENTIFIER, STRING, STRING_START, TUPLE, is_identifier, named,
    string_prefix,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const STRING_END: &str = "string_end";
const DICTIONARY: &str = "dictionary";
const SUBSCRIPT: &str = "subscript";
const BYTES_PREFIX: char = 'b';
const FORMATTED_PREFIX: char = 'f';
const TEMPLATE_PREFIX: char = 't';
const UNCONVERTIBLE: &[char] = &['d', 'i', 'u', 'c'];
const REPRESENTATIONS: &[char] = &['a', 'r'];
const STRING_CONVERSION: char = 's';
const PAIR: &str = "pair";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const KEY_FIELD: &str = "key";
const LITERALS: &[&str] = &[
    STRING,
    CONCATENATED_STRING,
    "integer",
    "float",
    "true",
    "false",
    "none",
    "ellipsis",
];
const VALUES: &[&str] = &[IDENTIFIER, ATTRIBUTE, SUBSCRIPT, CALL, TUPLE, DICTIONARY];

// UP031: `"…" % valores`, que pyupgrade pasa a `str.format`. `operation` es el `%`, `left` el
// string sin paréntesis y `right` los valores sin paréntesis.
pub fn printf_formatting(
    operation: Node<'_>,
    left: Node<'_>,
    right: Node<'_>,
    source: &[u8],
) -> Option<Report> {
    let parts = string_parts(left);
    if parts.iter().any(|part| !is_plain_string(*part, source)) {
        return None;
    }
    let is_dictionary = right.kind() == DICTIONARY;
    for part in parts {
        let specifiers = specifiers(&raw_contents(part, source)).ok()?;
        if !specifiers
            .iter()
            .all(|specifier| is_convertible(specifier, is_dictionary))
        {
            return Some(Report::at(PythonRule::UP031, left, ""));
        }
    }
    if is_dictionary && !has_clean_keys(right, source) {
        return Some(Report::at(PythonRule::UP031, left, ""));
    }
    (LITERALS.contains(&right.kind()) || VALUES.contains(&right.kind()))
        .then(|| Report::at(PythonRule::UP031, operation, ""))
}

fn string_parts(string: Node<'_>) -> Vec<Node<'_>> {
    if string.kind() == CONCATENATED_STRING {
        named(string)
            .into_iter()
            .filter(|part| part.kind() == STRING)
            .collect()
    } else {
        vec![string]
    }
}

// Ni bytes, ni f-string, ni t-string: esos no son un `ExprStringLiteral` de ruff.
fn is_plain_string(part: Node<'_>, source: &[u8]) -> bool {
    let prefix = string_prefix(part, source).to_ascii_lowercase();
    ![BYTES_PREFIX, FORMATTED_PREFIX, TEMPLATE_PREFIX]
        .iter()
        .any(|marker| prefix.contains(*marker))
}

// El texto entre las comillas tal como está escrito, con los escapes sin resolver.
fn raw_contents(part: Node<'_>, source: &[u8]) -> String {
    let start = part
        .child(0)
        .filter(|child| child.kind() == STRING_START)
        .map_or_else(|| part.start_byte(), |child| child.end_byte());
    let count = part.child_count();
    let end = count
        .checked_sub(1)
        .and_then(|last| part.child(last))
        .filter(|child| child.kind() == STRING_END)
        .map_or_else(|| part.end_byte(), |child| child.start_byte());
    source
        .get(start..end.max(start))
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default()
}

// `convertible` de pyupgrade: lo que `str.format` no puede escribir igual.
fn is_convertible(specifier: &Specifier, is_dictionary: bool) -> bool {
    let from_values = specifier.width == Some(Quantity::Star) || specifier.precision == Some(Quantity::Star);
    let nontrivial = specifier.flagged || specifier.width.is_some() || specifier.precision.is_some();
    !(from_values
        || UNCONVERTIBLE.contains(&specifier.conversion)
        || specifier.key.as_ref().is_some_and(String::is_empty)
        || (nontrivial && REPRESENTATIONS.contains(&specifier.conversion))
        || (specifier.width.is_some() && specifier.conversion == STRING_CONVERSION)
        || (is_dictionary && specifier.key.is_none()))
}

// `clean_params_dictionary` de pyupgrade: el diccionario pasa a argumentos con nombre si sus claves
// son strings identificadores sin repetir y, en varias líneas, alguna empieza su línea.
fn has_clean_keys(dictionary: Node<'_>, source: &[u8]) -> bool {
    let items: Vec<Node<'_>> = named(dictionary)
        .into_iter()
        .filter(|item| item.kind() == PAIR || item.kind() == DICTIONARY_SPLAT)
        .collect();
    let keys: Vec<Node<'_>> = items
        .iter()
        .filter(|item| item.kind() == PAIR)
        .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
        .collect();
    let mut seen: Vec<String> = Vec::new();
    for key in &keys {
        let Some(name) = string_key(*key, source).filter(|name| is_identifier(name) && !seen.contains(name))
        else {
            return false;
        };
        seen.push(name);
    }
    let multi_line = dictionary.start_position().row != dictionary.end_position().row;
    !items.is_empty() && (!multi_line || keys.iter().any(|key| starts_its_line(*key, source)))
}

fn string_key(key: Node<'_>, source: &[u8]) -> Option<String> {
    let plain = matches!(key.kind(), STRING | CONCATENATED_STRING)
        && string_parts(key)
            .iter()
            .all(|part| is_plain_string(*part, source));
    plain.then(|| text_value(key, source)).flatten()
}

fn starts_its_line(key: Node<'_>, source: &[u8]) -> bool {
    let start = key.start_byte();
    let line_start = source
        .get(..start)
        .and_then(|before| before.iter().rposition(|byte| *byte == b'\n'))
        .map_or(0, |newline| newline.saturating_add(1));
    source
        .get(line_start..start)
        .is_some_and(|indent| indent.iter().all(|byte| *byte == b' ' || *byte == b'\t'))
}
