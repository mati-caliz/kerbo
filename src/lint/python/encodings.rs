use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, NAME_FIELD,
    OBJECT_FIELD, STRING, VALUE_FIELD, is_bytes, is_f_string, is_string, named, string_prefix, text,
    unparenthesized,
};
use super::escapes::text_value;
use super::fix_edits::{Parentheses, pad, remove_argument};
use crate::fix::Fix;
use crate::rules::PythonRule;

const ENCODE: &str = "encode";
const UNICODE_PREFIXES: [char; 2] = ['u', 'U'];
const ENCODING_KEYWORD: &str = "encoding";
const UTF8_NAMES: [&str; 6] = ["utf-8", "utf8", "utf_8", "u8", "utf", "cp65001"];
const ARGUMENT_LIST: &str = "argument_list";
const RAW_PREFIXES: [char; 2] = ['r', 'R'];
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const TRIPLE_QUOTE_LENGTH: usize = 3;
const QUOTE_LENGTH: usize = 1;
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const BACKSLASH: char = '\\';
const STRING_ONLY_ESCAPES: [char; 3] = ['N', 'u', 'U'];
const HEX_ESCAPE: char = 'x';
const HEX_DIGITS: usize = 2;
const OCTAL_RADIX: u32 = 8;
const MAXIMUM_OCTAL_DIGITS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncodingArgument {
    Empty,
    Given,
}

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == CALL)
        .filter_map(|call| unnecessary_encode_utf8(*call, source))
        .collect()
}

// UP012: `"texto".encode("utf-8")`, donde alcanza con `b"texto"` (si es ASCII) o con `.encode()`.
fn unnecessary_encode_utf8(call: Node<'_>, source: &[u8]) -> Option<Report> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .filter(|attribute| text(*attribute, source) == ENCODE)?;
    let object = function.child_by_field_name(OBJECT_FIELD).map(unparenthesized)?;
    if !is_string(object) {
        return None;
    }
    let parts = if object.kind() == STRING {
        vec![object]
    } else {
        named(object)
    };
    let is_template = parts
        .iter()
        .any(|part| string_prefix(*part, source).contains(TEMPLATE_PREFIXES));
    if is_template || parts.iter().any(|part| is_bytes(*part, source)) {
        return None;
    }
    let encoding = encoding_argument(call, source)?;
    let formatted = parts.iter().any(|part| is_f_string(*part, source));
    let ascii = !formatted && text_value(object, source)?.is_ascii();
    let reported = if formatted {
        encoding == EncodingArgument::Given
    } else {
        let only_string_escapes = parts.iter().any(|part| has_string_only_escapes(*part, source));
        !only_string_escapes && (ascii || encoding == EncodingArgument::Given)
    };
    if !reported {
        return None;
    }
    let fix = if ascii {
        Some(bytes_literal_fix(call, function, &parts, source))
    } else {
        encoding_removal(call, source)
    };
    Some(Report::at(PythonRule::UP012, call, "").with_fix(fix))
}

// `"texto".encode()` pasa a `b"texto"`: cada parte del string con `b` adelante y sin la `u`.
fn bytes_literal_fix(call: Node<'_>, function: Node<'_>, parts: &[Node<'_>], source: &[u8]) -> Fix {
    let object = function.child_by_field_name(OBJECT_FIELD).unwrap_or(function);
    let mut replacement = String::new();
    let mut previous = object.start_byte();
    for part in parts {
        replacement.push_str(&String::from_utf8_lossy(
            source.get(previous..part.start_byte()).unwrap_or_default(),
        ));
        replacement.push('b');
        replacement.push_str(text(*part, source).trim_start_matches(UNICODE_PREFIXES));
        previous = part.end_byte();
    }
    replacement.push_str(&String::from_utf8_lossy(
        source.get(previous..object.end_byte()).unwrap_or_default(),
    ));
    Fix::replace(call, &pad(replacement, call, source), source)
}

// Sacar el `"utf-8"` (posicional o `encoding=`), que es el valor por defecto.
fn encoding_removal(call: Node<'_>, source: &[u8]) -> Option<Fix> {
    let list = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let argument = named(list)
        .into_iter()
        .find(|argument| argument.kind() != COMMENT)?;
    Some(remove_argument(argument, list, Parentheses::Preserve, source))
}

// `match_encoding_arg`: sin argumentos, o un único `"utf-8"` posicional o como `encoding=`.
fn encoding_argument(call: Node<'_>, source: &[u8]) -> Option<EncodingArgument> {
    let list = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|list| list.kind() == ARGUMENT_LIST)?;
    let arguments: Vec<Node<'_>> = named(list)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    match arguments.as_slice() {
        [] => Some(EncodingArgument::Empty),
        [keyword] if keyword.kind() == KEYWORD_ARGUMENT => {
            let is_encoding = keyword
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == ENCODING_KEYWORD);
            let value = keyword.child_by_field_name(VALUE_FIELD)?;
            (is_encoding && is_utf8_name(value, source)).then_some(EncodingArgument::Given)
        }
        [positional] => is_utf8_name(*positional, source).then_some(EncodingArgument::Given),
        _ => None,
    }
}

fn is_utf8_name(value: Node<'_>, source: &[u8]) -> bool {
    let value = unparenthesized(value);
    let parts = if value.kind() == STRING {
        vec![value]
    } else {
        named(value)
    };
    let is_template = parts
        .iter()
        .any(|part| string_prefix(*part, source).contains(TEMPLATE_PREFIXES));
    is_string(value)
        && !is_template
        && text_value(value, source).is_some_and(|name| UTF8_NAMES.contains(&name.to_lowercase().as_str()))
}

// `string_contains_string_only_escapes`: un escape que sólo existe en un str (`\N{…}`, `\u`, `\U` u
// octal mayor a 255), que en un bytes significaría otra cosa.
fn has_string_only_escapes(part: Node<'_>, source: &[u8]) -> bool {
    let prefix = string_prefix(part, source);
    if prefix.contains(RAW_PREFIXES) {
        return false;
    }
    let literal = text(part, source).get(prefix.len()..).unwrap_or_default();
    let quote = if TRIPLE_QUOTES.iter().any(|quote| literal.starts_with(quote)) {
        TRIPLE_QUOTE_LENGTH
    } else {
        QUOTE_LENGTH
    };
    let content = literal
        .get(quote..literal.len().saturating_sub(quote))
        .unwrap_or_default();
    contains_string_only_escape(content)
}

fn contains_string_only_escape(content: &str) -> bool {
    let mut rest = content;
    while let Some(position) = rest.find(BACKSLASH) {
        rest = rest.get(position.saturating_add(1)..).unwrap_or_default();
        let Some(escaped) = rest.chars().next() else {
            continue;
        };
        rest = rest.get(escaped.len_utf8()..).unwrap_or_default();
        if STRING_ONLY_ESCAPES.contains(&escaped) {
            return true;
        }
        if escaped == HEX_ESCAPE {
            rest = rest.get(HEX_DIGITS..).unwrap_or_default();
        } else if escaped.is_digit(OCTAL_RADIX) {
            let following: String = rest
                .chars()
                .take(MAXIMUM_OCTAL_DIGITS.saturating_sub(1))
                .take_while(|digit| digit.is_digit(OCTAL_RADIX))
                .collect();
            let octal = format!("{escaped}{following}");
            if u8::from_str_radix(&octal, OCTAL_RADIX).is_err() {
                return true;
            }
            rest = rest.get(following.len()..).unwrap_or_default();
        }
    }
    false
}
