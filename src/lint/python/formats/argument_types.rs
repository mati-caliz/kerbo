use std::collections::HashMap;

use tree_sitter::Node;

use super::percent::{Specifier, specifiers};
use crate::lint::python::ast::{
    CONCATENATED_STRING, NOT_OPERATOR, STRING, TUPLE, is_bytes, is_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;

const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const UNARY: &str = "unary_operator";
const CONDITIONAL: &str = "conditional_expression";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const SIGNS: [&str; 2] = ["-", "+"];
const INVERT: &str = "~";
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const CHARACTER: char = 'c';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueType {
    Text,
    Integer,
    Float,
    Complex,
    Boolean,
    Container,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatType {
    Text,
    Integer,
    Float,
    Number,
    Unknown,
}

impl FormatType {
    const fn from_conversion(conversion: char) -> Self {
        match conversion {
            'r' | 's' => Self::Text,
            'n' | 'd' => Self::Number,
            'b' | 'c' | 'o' | 'x' | 'X' => Self::Integer,
            'e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%' => Self::Float,
            _ => Self::Unknown,
        }
    }

    const fn accepts(self, value: ValueType) -> bool {
        match value {
            ValueType::Text | ValueType::Container | ValueType::Complex => {
                matches!(self, Self::Unknown | Self::Text)
            }
            ValueType::Integer | ValueType::Boolean => true,
            ValueType::Float => !matches!(self, Self::Integer),
        }
    }
}

// PLE1307: `"%d" % "1"`, un argumento literal que no sirve para su conversión. Como el
// `bad_string_format_type` de ruff: una tupla se compara elemento a elemento, un diccionario por
// clave y cualquier otro valor sólo si el formato tiene una única conversión.
pub fn mismatched_types(format: &str, right: Node<'_>, source: &[u8]) -> bool {
    let Ok(found) = specifiers(format) else {
        return false;
    };
    match right.kind() {
        TUPLE => {
            let elements = named(right);
            found.len() <= elements.len()
                && found
                    .iter()
                    .zip(&elements)
                    .any(|(specifier, element)| !compatible(specifier, *element, source))
        }
        DICTIONARY => mismatched_mapping(&found, right, source),
        _ => match found.as_slice() {
            [only] => !compatible(only, right, source),
            _ => false,
        },
    }
}

fn mismatched_mapping(found: &[Specifier], dictionary: Node<'_>, source: &[u8]) -> bool {
    let items = named(dictionary);
    if found.len() > items.len() {
        return false;
    }
    let by_key: HashMap<&str, &Specifier> = found
        .iter()
        .filter_map(|specifier| specifier.key.as_deref().map(|key| (key, specifier)))
        .collect();
    for item in items {
        if item.kind() != PAIR {
            return false;
        }
        let (Some(key), Some(value)) = (
            item.child_by_field_name(KEY_FIELD),
            item.child_by_field_name(VALUE_FIELD),
        ) else {
            return false;
        };
        if key.kind() != STRING || is_bytes(key, source) {
            return false;
        }
        let Some(specifier) = text_value(key, source).and_then(|key| by_key.get(key.as_str()).copied())
        else {
            return false;
        };
        if !compatible(specifier, value, source) {
            return true;
        }
    }
    false
}

fn compatible(specifier: &Specifier, value: Node<'_>, source: &[u8]) -> bool {
    let value = unparenthesized(value);
    let format_type = FormatType::from_conversion(specifier.conversion);
    let types = value_types(value, source);
    if types.is_empty() {
        return true;
    }
    if specifier.conversion == CHARACTER
        && value.kind() == STRING
        && !is_bytes(value, source)
        && text_value(value, source).is_some_and(|content| content.chars().count() == 1)
    {
        return true;
    }
    types
        .into_iter()
        .all(|value_type| format_type.accepts(value_type))
}

// `ResolvedPythonType::from` de ruff para literales; lo que no se puede resolver queda vacío.
fn value_types(value: Node<'_>, source: &[u8]) -> Vec<ValueType> {
    let value = unparenthesized(value);
    match value.kind() {
        STRING | CONCATENATED_STRING if is_string(value) => vec![ValueType::Text],
        "integer" | "float" if text(value, source).ends_with(IMAGINARY_SUFFIXES) => vec![ValueType::Complex],
        "integer" => vec![ValueType::Integer],
        "float" => vec![ValueType::Float],
        "true" | "false" | NOT_OPERATOR => vec![ValueType::Boolean],
        "none"
        | "ellipsis"
        | "list"
        | "list_comprehension"
        | "dictionary"
        | "dictionary_comprehension"
        | "set"
        | "set_comprehension"
        | TUPLE
        | "generator_expression" => vec![ValueType::Container],
        UNARY => unary_types(value, source),
        CONDITIONAL => {
            let parts = named(value);
            let (Some(body), Some(orelse)) = (parts.first(), parts.get(2)) else {
                return Vec::new();
            };
            let mut types = value_types(*body, source);
            let other = value_types(*orelse, source);
            if types.is_empty() || other.is_empty() {
                return Vec::new();
            }
            types.extend(other);
            types
        }
        _ => Vec::new(),
    }
}

fn unary_types(value: Node<'_>, source: &[u8]) -> Vec<ValueType> {
    let Some(operand) = value.child_by_field_name(ARGUMENT_FIELD) else {
        return Vec::new();
    };
    let operator = value
        .child_by_field_name(OPERATOR_FIELD)
        .map(|operator| text(operator, source))
        .unwrap_or_default();
    let inner = value_types(operand, source);
    match inner.as_slice() {
        [ValueType::Boolean | ValueType::Integer] if SIGNS.contains(&operator) || operator == INVERT => {
            vec![ValueType::Integer]
        }
        [number @ (ValueType::Float | ValueType::Complex)] if SIGNS.contains(&operator) => vec![*number],
        _ => Vec::new(),
    }
}
