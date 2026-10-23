use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, NAME_FIELD,
    NONE, OBJECT_FIELD, STRING, VALUE_FIELD, is_string, named, string_prefix, text, unparenthesized,
};
use super::escapes::text_value;
use crate::rules::PythonRule;

const SPLIT_METHODS: [&str; 2] = ["split", "rsplit"];
const SEPARATOR_KEYWORD: &str = "sep";
const MAXSPLIT_KEYWORD: &str = "maxsplit";
const SEPARATOR_POSITION: usize = 0;
const MAXSPLIT_POSITION: usize = 1;
const INTEGER: &str = "integer";
const UNARY: &str = "unary_operator";
const MINUS: &str = "-";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const LIST_SPLAT: &str = "list_splat";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const DIGIT_SEPARATOR: char = '_';
const RADIX_PREFIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];
const DECIMAL_RADIX: u32 = 10;

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == CALL)
        .filter_map(|call| split_static_string(*call, source))
        .collect()
}

// SIM905: `"a b c".split()` sobre un string fijo, que se escribe como la lista. Como ruff, sólo con
// un separador que falta, es `None` o es un string, y un `maxsplit` que falta o es un entero escrito.
fn split_static_string(call: Node<'_>, source: &[u8]) -> Option<Report> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .filter(|attribute| SPLIT_METHODS.contains(&text(*attribute, source)))?;
    function
        .child_by_field_name(OBJECT_FIELD)
        .map(unparenthesized)
        .filter(|object| is_plain_string(*object, source))?;
    let arguments: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    let maxsplit = argument_value(&arguments, MAXSPLIT_KEYWORD, MAXSPLIT_POSITION, source);
    if maxsplit.is_some_and(|value| !is_small_integer(value, source)) {
        return None;
    }
    let separator = argument_value(&arguments, SEPARATOR_KEYWORD, SEPARATOR_POSITION, source);
    let static_separator =
        separator.is_none_or(|value| value.kind() == NONE || is_plain_string(value, source));
    static_separator.then(|| Report::at(PythonRule::SIM905, call, ""))
}

fn is_plain_string(node: Node<'_>, source: &[u8]) -> bool {
    let parts = if node.kind() == STRING {
        vec![node]
    } else {
        named(node)
    };
    is_string(node)
        && text_value(node, source).is_some()
        && !parts
            .iter()
            .any(|part| string_prefix(*part, source).contains(TEMPLATE_PREFIXES))
}

// `Arguments::find_argument_value`: el argumento con ese nombre o, si no, el de esa posición entre
// los posicionales anteriores al primer `*`.
fn argument_value<'tree>(
    arguments: &[Node<'tree>],
    keyword: &str,
    position: usize,
    source: &[u8],
) -> Option<Node<'tree>> {
    let by_name = arguments
        .iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .find(|argument| {
            argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == keyword)
        })
        .and_then(|argument| argument.child_by_field_name(VALUE_FIELD));
    by_name
        .or_else(|| {
            arguments
                .iter()
                .filter(|argument| !matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT))
                .take_while(|argument| argument.kind() != LIST_SPLAT)
                .nth(position)
                .copied()
        })
        .map(unparenthesized)
}

// `get_maxsplit_value`: un entero escrito, con un `-` opcional, que entra en 32 bits.
fn is_small_integer(value: Node<'_>, source: &[u8]) -> bool {
    let literal = if value.kind() == UNARY {
        let is_minus = value
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == MINUS);
        value
            .child_by_field_name(ARGUMENT_FIELD)
            .map(unparenthesized)
            .filter(|operand| is_minus && operand.kind() == INTEGER)
    } else {
        Some(value).filter(|literal| literal.kind() == INTEGER)
    };
    literal.is_some_and(|literal| fits_in_i32(text(literal, source)))
}

fn fits_in_i32(literal: &str) -> bool {
    let digits = literal.replace(DIGIT_SEPARATOR, "").to_ascii_lowercase();
    let (radix, digits) = RADIX_PREFIXES
        .iter()
        .find_map(|(prefix, radix)| digits.strip_prefix(prefix).map(|rest| (*radix, rest.to_owned())))
        .unwrap_or_else(|| (DECIMAL_RADIX, digits.clone()));
    i32::from_str_radix(&digits, radix).is_ok()
}
