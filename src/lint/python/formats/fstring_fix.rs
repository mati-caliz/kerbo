use tree_sitter::Node;

use super::fstring::{Arguments, format_arguments, format_object};
use super::printf_format::curly_escape;
use super::template::{self, FieldPart, Piece, Replacement};
use crate::fix::Fix;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, FUNCTION_FIELD, OBJECT_FIELD, STRING, named, text, unparenthesized,
};
use crate::lint::python::fix_edits::has_comment_within;

const DOT: &str = ".";
const UNICODE_PREFIXES: [char; 2] = ['u', 'U'];
const RAW_PREFIXES: [char; 2] = ['r', 'R'];
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const DOUBLE_QUOTE: &str = "\"";
const TRIPLE_QUOTE_LENGTH: usize = 3;
const QUOTE_LENGTH: usize = 1;
const DIGIT_SEPARATOR: char = '_';
const DOUBLED_OPENING: &str = "{{";
const DOUBLED_CLOSING: &str = "}}";
const ACCESSED_PARENTHESIZED: [&str; 10] = [
    "binary_operator",
    "unary_operator",
    "not_operator",
    "boolean_operator",
    "comparison_operator",
    "conditional_expression",
    "lambda",
    "await",
    "yield",
    "list_splat",
];
const ALWAYS_PARENTHESIZED: [&str; 6] = [
    "generator_expression",
    "dictionary",
    "named_expression",
    "set",
    "set_comprehension",
    "dictionary_comprehension",
];
const CONTAINERS: [&str; 5] = [
    "generator_expression",
    "dictionary",
    "set",
    "set_comprehension",
    "dictionary_comprehension",
];
const INTEGER: &str = "integer";
const SUBSCRIPT: &str = "subscript";
const ATTRIBUTE: &str = "attribute";
const CALL: &str = "call";
const VALUE_FIELD: &str = "value";
const LAMBDA: &str = "lambda";
const CONVERSIONS: [char; 3] = ['s', 'r', 'a'];

enum Conversion {
    EmptyLiteral,
    NonEmptyLiteral,
    Convert(String),
}

// El arreglo de UP032 de ruff: cada parte del string pasa a f-string con los argumentos adentro y
// se borra el `.format(...)`. Sin arreglo si hay comentarios entre los argumentos.
pub fn fstring_fix(call: Node<'_>, source: &[u8]) -> Option<Fix> {
    let object = format_object(call, source)?;
    let arguments = format_arguments(call, source)?;
    let list = call.child_by_field_name(ARGUMENTS_FIELD)?;
    if has_comment_within(list, list.start_byte(), list.end_byte()) {
        return None;
    }
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let dot = {
        let mut cursor = function.walk();
        let found = function.children(&mut cursor).find(|child| child.kind() == DOT);
        found?.start_byte()
    };
    let segments = if object.kind() == STRING {
        vec![object]
    } else {
        named(object)
            .into_iter()
            .filter(|segment| segment.kind() == STRING)
            .collect()
    };
    let mut next_auto = 0;
    let mut conversions = Vec::new();
    for segment in &segments {
        conversions.push((
            *segment,
            converted(text(*segment, source), &arguments, &mut next_auto, source)?,
        ));
    }
    let mut contents = String::new();
    let mut previous_end = call.start_byte();
    for (segment, conversion) in conversions {
        let replacement = match conversion {
            Conversion::Convert(fstring) => fstring,
            Conversion::EmptyLiteral => {
                if !contents.is_empty() {
                    previous_end = segment.end_byte();
                }
                continue;
            }
            Conversion::NonEmptyLiteral => text(segment, source)
                .replace(DOUBLED_OPENING, "{")
                .replace(DOUBLED_CLOSING, "}"),
        };
        contents.push_str(&String::from_utf8_lossy(
            source.get(previous_end..segment.start_byte())?,
        ));
        contents.push_str(&replacement);
        previous_end = segment.end_byte();
    }
    contents.push_str(&String::from_utf8_lossy(source.get(previous_end..dot)?));
    let follows_letter = source
        .get(..call.start_byte())
        .and_then(|before| before.last())
        .is_some_and(u8::is_ascii_alphabetic);
    if follows_letter {
        contents.insert(0, ' ');
    }
    if contents.is_empty() {
        text(object, source).clone_into(&mut contents);
    }
    Some(Fix::replace(call, &contents, source))
}

// `FStringConversion::try_convert` de ruff sobre una parte del string, con su prefijo y comillas.
fn converted(
    segment: &str,
    arguments: &Arguments<'_, '_>,
    next_auto: &mut usize,
    source: &[u8],
) -> Option<Conversion> {
    let unprefixed = segment.strip_prefix(UNICODE_PREFIXES).unwrap_or(segment);
    let raw = unprefixed.starts_with(RAW_PREFIXES);
    let quoted = unprefixed.strip_prefix(RAW_PREFIXES).unwrap_or(unprefixed);
    let quote_length = if TRIPLE_QUOTES.iter().any(|quote| quoted.starts_with(quote)) {
        TRIPLE_QUOTE_LENGTH
    } else {
        QUOTE_LENGTH
    };
    let quote = quoted.get(..quote_length)?;
    let contents = quoted.get(quote_length..quoted.len().saturating_sub(quote_length))?;
    if contents.is_empty() {
        return Some(Conversion::EmptyLiteral);
    }
    let pieces = template::pieces(contents, raw)?;
    if pieces.iter().all(|piece| matches!(piece, Piece::Literal(_))) {
        return Some(Conversion::NonEmptyLiteral);
    }
    let mut converted = String::new();
    for piece in pieces {
        match piece {
            Piece::Literal(value) => converted.push_str(&curly_escape(&value)),
            Piece::Field(replacement) => {
                let field = template::field_type(&replacement.field_name)?;
                let (_, value) = arguments.find(&field, next_auto)?;
                converted.push_str(&converted_field(&replacement, value, quote, source)?);
            }
        }
    }
    let raw_prefix = if raw { "r" } else { "" };
    Some(Conversion::Convert(format!(
        "{raw_prefix}f{quote}{converted}{quote}"
    )))
}

// Un campo `{…}` con el argumento adentro, sus accesos, la conversión y el especificador.
fn converted_field(replacement: &Replacement, value: Node<'_>, quote: &str, source: &[u8]) -> Option<String> {
    let parts = template::field_parts(&replacement.field_name);
    let expression = formatted_expression(value, !parts.is_empty(), source);
    if breaks_fstring(value, parts.is_empty(), replacement.conversion) || expression.starts_with('{') {
        return None;
    }
    let mut field = String::from("{");
    field.push_str(&expression);
    for part in parts {
        match part {
            FieldPart::Attribute(name) => {
                field.push('.');
                field.push_str(&name);
            }
            FieldPart::Index(index) => {
                field.push('[');
                field.push_str(&index.to_string());
                field.push(']');
            }
            FieldPart::StringIndex(index) => {
                let inner = if quote == DOUBLE_QUOTE { '\'' } else { '"' };
                field.push('[');
                field.push(inner);
                field.push_str(&index);
                field.push(inner);
                field.push(']');
            }
        }
    }
    if let Some(conversion) = replacement.conversion {
        field.push('!');
        field.push(conversion);
    }
    if !replacement.format_spec.is_empty() {
        field.push(':');
        field.push_str(&replacement.format_spec);
    }
    field.push('}');
    Some(field)
}

// Lo que ruff deja como f-string inválida y por eso revierte el archivo entero: una lambda suelta
// (`f"{lambda: x}"`), una conversión que no es `!s`, `!r` ni `!a`, o una expresión que empieza con
// llave (`f"{{x: x}.get(1)}"`). Kerbo no ofrece el arreglo.
fn breaks_fstring(value: Node<'_>, bare: bool, conversion: Option<char>) -> bool {
    (bare && value.kind() == LAMBDA)
        || conversion.is_some_and(|conversion| !CONVERSIONS.contains(&conversion))
}

// `formatted_expr` de ruff: entre paréntesis lo que no se puede escribir suelto en la f-string.
fn formatted_expression(value: Node<'_>, accessed: bool, source: &[u8]) -> String {
    let value_text = text(value, source);
    let kind = value.kind();
    let inner_container = |field: &str| {
        value
            .child_by_field_name(field)
            .map(unparenthesized)
            .is_some_and(|inner| CONTAINERS.contains(&inner.kind()))
    };
    let parenthesize = (accessed && ACCESSED_PARENTHESIZED.contains(&kind))
        || (accessed
            && kind == INTEGER
            && value_text
                .chars()
                .filter(|character| *character != DIGIT_SEPARATOR)
                .all(|character| character.is_ascii_digit()))
        || ALWAYS_PARENTHESIZED.contains(&kind)
        || (kind == SUBSCRIPT && inner_container(VALUE_FIELD))
        || (kind == ATTRIBUTE && inner_container(OBJECT_FIELD))
        || (kind == CALL && inner_container(FUNCTION_FIELD));
    if parenthesize && !(value_text.starts_with('(') && value_text.ends_with(')')) {
        format!("({value_text})")
    } else {
        value_text.to_owned()
    }
}
