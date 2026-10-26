use tree_sitter::Node;

use super::comprehension_fixes::preferred_quote;
use super::model::Model;
use super::qualified::is_builtin_named;
use super::tail_call_fixes::expression_parent;
use crate::fix::Fix;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, COMMENT, CONCATENATED_STRING, FALSE, FUNCTION_FIELD, INTERPOLATION, KEYWORD_ARGUMENT,
    NAME_FIELD, STRING, TRUE, VALUE_FIELD, named, string_prefix, text,
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

const ATTRIBUTE: &str = "attribute";
const LINE_BREAKS: [char; 2] = ['\n', '\r'];
const POWER: &str = "**";
const BINARY: &str = "binary_operator";
// Lo que en ruff tiene más precedencia que un `-1`: los átomos, las llamadas, los atributos, los
// subíndices, `await` y `**`.
const ABOVE_UNARY: [&str; 15] = [
    "list",
    "tuple",
    "expression_list",
    "set",
    "dictionary",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
    "call",
    "attribute",
    "subscript",
    "await",
    "interpolation",
    "slice",
];

// El arreglo de UP018: el literal de adentro, o el valor cero del tipo sin argumentos. Entre
// paréntesis si es un `int` al que se le pide un atributo, si un signo o un string concatenado ocupa
// más de una línea, o si un signo queda adentro de algo que liga más fuerte.
pub fn native_literal_fix(call: Node<'_>, model: &Model<'_>, source: &[u8]) -> Option<Fix> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let builtin = NATIVE_TYPES
        .into_iter()
        .find(|builtin| is_builtin_named(model, function, source, builtin))?;
    let given: Vec<Node<'_>> = named(arguments)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    let argument = match given.as_slice() {
        [] => None,
        [keyword] if keyword.kind() == KEYWORD_ARGUMENT => keyword.child_by_field_name(VALUE_FIELD),
        [argument] => Some(*argument),
        _ => return None,
    };
    let parent = expression_parent(call);
    let under_attribute = builtin == INT && parent.is_some_and(|parent| parent.kind() == ATTRIBUTE);
    let mut content = argument.map_or_else(
        || zero_value(builtin, call, source),
        |argument| literal_content(argument, parent, source),
    );
    if under_attribute && !content.starts_with('(') {
        content = format!("({content})");
    }
    if source.get(call.end_byte()).is_some_and(u8::is_ascii_alphabetic) {
        content.push(' ');
    }
    Some(Fix::replace(call, &content, source))
}

fn literal_content(argument: Node<'_>, parent: Option<Node<'_>>, source: &[u8]) -> String {
    let written = text(argument, source);
    let multiline = written.contains(LINE_BREAKS);
    let is_unary = argument.kind() == UNARY;
    let binds_tighter = parent.is_some_and(|parent| {
        ABOVE_UNARY.contains(&parent.kind())
            || (parent.kind() == BINARY
                && parent
                    .child_by_field_name(OPERATOR_FIELD)
                    .is_some_and(|operator| text(operator, source) == POWER))
    });
    let needs_parentheses =
        (is_unary && (multiline || binds_tighter)) || (argument.kind() == CONCATENATED_STRING && multiline);
    if needs_parentheses {
        format!("({written})")
    } else {
        written.to_owned()
    }
}

fn zero_value(builtin: &str, call: Node<'_>, source: &[u8]) -> String {
    let quote = preferred_quote(call, source);
    match builtin {
        STR => format!("{quote}{quote}"),
        BYTES => format!("b{quote}{quote}"),
        INT => String::from("0"),
        FLOAT => String::from("0.0"),
        BOOL => String::from("False"),
        _ => String::from("0j"),
    }
}

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
