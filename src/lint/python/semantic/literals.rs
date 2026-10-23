use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::ast::{
    COMMENT, CONCATENATED_STRING, FALSE, INTERPOLATION, KEYWORD_ARGUMENT, NAME_FIELD, STRING, TRUE,
    VALUE_FIELD, named, string_prefix, text,
};

const STR: &str = "str";
const BYTES: &str = "bytes";
const INT: &str = "int";
const FLOAT: &str = "float";
const BOOL: &str = "bool";
const COMPLEX: &str = "complex";
const NATIVE_TYPES: [&str; 6] = [STR, BYTES, INT, FLOAT, BOOL, COMPLEX];
const REDUNDANT_KEYWORDS: [(&str, &str); 2] = [(STR, "object"), (COMPLEX, "real")];
const INTEGER: &str = "integer";
const FLOAT_LITERAL: &str = "float";
const UNARY: &str = "unary_operator";
const SIGNS: [&str; 2] = ["+", "-"];
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const BYTES_PREFIX: char = 'b';
const TEMPLATE_PREFIXES: [char; 2] = ['f', 't'];
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];

// UP018: `str("x")`, `int(1)`, `bool()` y compañía, donde alcanza con el literal.
pub fn is_native_literal_call(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    arguments: Node<'_>,
) -> bool {
    let Some(builtin) = NATIVE_TYPES
        .into_iter()
        .find(|builtin| is_builtin_named(model, function, source, builtin))
    else {
        return false;
    };
    let given: Vec<Node<'_>> = named(arguments)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    match given.as_slice() {
        [] => true,
        [keyword] if keyword.kind() == KEYWORD_ARGUMENT => {
            is_redundant_keyword(builtin, *keyword, source)
                && keyword
                    .child_by_field_name(VALUE_FIELD)
                    .is_some_and(|value| literal_type(value, source) == Some(builtin))
        }
        [argument] => literal_type(*argument, source) == Some(builtin),
        _ => false,
    }
}

fn is_redundant_keyword(builtin: &str, keyword: Node<'_>, source: &[u8]) -> bool {
    keyword
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| REDUNDANT_KEYWORDS.contains(&(builtin, text(name, source))))
}

fn literal_type(argument: Node<'_>, source: &[u8]) -> Option<&'static str> {
    match argument.kind() {
        STRING => string_type(argument, source),
        CONCATENATED_STRING => {
            let parts: Vec<Option<&'static str>> = named(argument)
                .into_iter()
                .map(|part| string_type(part, source))
                .collect();
            let first = parts.first().copied().flatten();
            parts.iter().all(|part| *part == first).then_some(first).flatten()
        }
        TRUE | FALSE => Some(BOOL),
        UNARY => {
            let sign = argument
                .child_by_field_name(OPERATOR_FIELD)
                .map(|operator| text(operator, source));
            let operand = argument.child_by_field_name(ARGUMENT_FIELD)?;
            (sign.is_some_and(|sign| SIGNS.contains(&sign))
                && matches!(operand.kind(), INTEGER | FLOAT_LITERAL))
            .then(|| number_type(operand, source))
        }
        INTEGER | FLOAT_LITERAL => Some(number_type(argument, source)),
        _ => None,
    }
}

fn string_type(string: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let prefix = string_prefix(string, source).to_ascii_lowercase();
    let interpolated =
        prefix.contains(TEMPLATE_PREFIXES) || named(string).iter().any(|part| part.kind() == INTERPOLATION);
    if interpolated {
        return None;
    }
    Some(if prefix.contains(BYTES_PREFIX) { BYTES } else { STR })
}

fn number_type(number: Node<'_>, source: &[u8]) -> &'static str {
    if text(number, source).ends_with(IMAGINARY_SUFFIXES) {
        COMPLEX
    } else if number.kind() == INTEGER {
        INT
    } else {
        FLOAT
    }
}
