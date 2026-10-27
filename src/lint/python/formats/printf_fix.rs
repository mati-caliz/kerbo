use tree_sitter::Node;

use super::printf_format::{Part, converted, parse};
use crate::fix::Fix;
use crate::lint::python::ast::{
    CONCATENATED_STRING, IDENTIFIER, STRING, STRING_START, TUPLE, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::indentation_unit;

const STRING_END: &str = "string_end";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const ATTRIBUTE: &str = "attribute";
const SUBSCRIPT: &str = "subscript";
const CALL: &str = "call";
const LINE_FEED: char = '\n';
const COMMA: char = ',';
const CLOSING_PARENTHESIS: char = ')';
const RIGHT_PARENTHESIS: u8 = b')';
const MODULO: u8 = b'%';
const HASH: u8 = b'#';

// El arreglo de UP031 de ruff: cada parte del string pasa al formato de `str.format` y los valores
// a los argumentos de `.format(...)`. Siempre inseguro.
pub fn printf_fix(operation: Node<'_>, left: Node<'_>, right: Node<'_>, source: &[u8]) -> Option<Fix> {
    let parts = if left.kind() == CONCATENATED_STRING {
        named(left)
            .into_iter()
            .filter(|part| part.kind() == STRING)
            .collect()
    } else {
        vec![left]
    };
    let mut positional = 0_usize;
    let mut keywords = 0_usize;
    let mut contents = String::new();
    let mut previous_end = operation.start_byte();
    for part in &parts {
        let (opening_end, closing_start) = delimiters(*part);
        let raw = String::from_utf8_lossy(source.get(opening_end..closing_start)?).into_owned();
        let parsed = parse(&raw)?;
        for spec in parsed.iter().filter_map(|piece| match piece {
            Part::Spec(spec) => Some(spec),
            Part::Literal(_) => None,
        }) {
            if spec.is_named() {
                keywords = keywords.saturating_add(1);
            } else {
                positional = positional.saturating_add(1);
            }
        }
        contents.push_str(&String::from_utf8_lossy(source.get(previous_end..opening_end)?));
        contents.extend(parsed.iter().map(converted));
        contents.push_str(&String::from_utf8_lossy(
            source.get(closing_start..part.end_byte())?,
        ));
        previous_end = part.end_byte();
    }
    contents.push_str(&kept_parentheses(source, previous_end));
    let arguments = parameters(right, positional, keywords, source)?;
    contents.push_str(".format");
    contents.push_str(&arguments);
    Some(Fix::replace(operation, &contents, source).marked_unsafe())
}

fn delimiters(part: Node<'_>) -> (usize, usize) {
    let opening_end = part
        .child(0)
        .filter(|child| child.kind() == STRING_START)
        .map_or_else(|| part.start_byte(), |child| child.end_byte());
    let closing_start = part
        .child_count()
        .checked_sub(1)
        .and_then(|last| part.child(last))
        .filter(|child| child.kind() == STRING_END)
        .map_or_else(|| part.end_byte(), |child| child.start_byte());
    (opening_end, closing_start.max(opening_end))
}

// Como ruff, por cada `)` entre el string y el `%` se repite el texto desde el final del string
// hasta ese paréntesis.
fn kept_parentheses(source: &[u8], string_end: usize) -> String {
    let mut kept = String::new();
    let mut offset = string_end;
    while let Some(byte) = source.get(offset).copied() {
        match byte {
            MODULO => break,
            HASH => {
                offset = source
                    .get(offset..)
                    .and_then(|rest| rest.iter().position(|next| *next == b'\n'))
                    .map_or(source.len(), |newline| offset.saturating_add(newline));
                continue;
            }
            RIGHT_PARENTHESIS => {
                kept.push_str(&String::from_utf8_lossy(
                    source
                        .get(string_end..offset.saturating_add(1))
                        .unwrap_or_default(),
                ));
            }
            _ => {}
        }
        offset = offset.saturating_add(1);
    }
    kept
}

fn parameters(right: Node<'_>, positional: usize, keywords: usize, source: &[u8]) -> Option<String> {
    let right_text = text(right, source);
    match right.kind() {
        IDENTIFIER | ATTRIBUTE | SUBSCRIPT | CALL if keywords > 0 => Some(format!("(**{right_text})")),
        IDENTIFIER | ATTRIBUTE | SUBSCRIPT | CALL if positional > 1 => Some(format!("(*{right_text})")),
        TUPLE => Some(tuple_parameters(right, source)),
        DICTIONARY => dictionary_parameters(right, source),
        _ => Some(format!("({right_text})")),
    }
}

// `clean_params_tuple`: una tupla de un elemento en una línea pierde su coma.
fn tuple_parameters(tuple: Node<'_>, source: &[u8]) -> String {
    let contents = text(tuple, source);
    let single = named(tuple).len() == 1 && !contents.contains(LINE_FEED);
    contents.rfind(COMMA).filter(|_| single).map_or_else(
        || contents.to_owned(),
        |comma| {
            format!(
                "{}{}",
                contents.get(..comma).unwrap_or_default(),
                contents.get(comma.saturating_add(1)..).unwrap_or_default()
            )
        },
    )
}

// `clean_params_dictionary`: cada clave pasa a un argumento con nombre; en varias líneas, uno por
// línea con la sangría de la primera clave que empieza su línea.
fn dictionary_parameters(dictionary: Node<'_>, source: &[u8]) -> Option<String> {
    let multi_line = dictionary.start_position().row != dictionary.end_position().row;
    let mut arguments = Vec::new();
    let mut indentation: Option<String> = None;
    for item in named(dictionary) {
        match item.kind() {
            PAIR => {
                let key = item.child_by_field_name(KEY_FIELD)?;
                let value = item.child_by_field_name(VALUE_FIELD)?;
                if multi_line && indentation.is_none() {
                    indentation = line_indentation(key, source);
                }
                let name = text_value(key, source)?;
                arguments.push(format!("{name}={}", text(unparenthesized(value), source)));
            }
            DICTIONARY_SPLAT => {
                let value = named(item).into_iter().next()?;
                arguments.push(format!("**{}", text(unparenthesized(value), source)));
            }
            _ => {}
        }
    }
    if arguments.is_empty() {
        return None;
    }
    if !multi_line {
        return Some(format!("({})", arguments.join(", ")));
    }
    let indentation = indentation?;
    let mut contents = String::from("(");
    for argument in &arguments {
        contents.push(LINE_FEED);
        contents.push_str(&indentation);
        contents.push_str(argument);
        contents.push(COMMA);
    }
    contents.push(LINE_FEED);
    let unit = indentation_unit(dictionary, source);
    contents.push_str(indentation.strip_prefix(unit.as_str()).unwrap_or(&indentation));
    contents.push(CLOSING_PARENTHESIS);
    Some(contents)
}

fn line_indentation(node: Node<'_>, source: &[u8]) -> Option<String> {
    let start = node.start_byte();
    let line_start = source
        .get(..start)
        .and_then(|before| before.iter().rposition(|byte| *byte == b'\n'))
        .map_or(0, |newline| newline.saturating_add(1));
    let before = source.get(line_start..start)?;
    before
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\x0c'))
        .then(|| String::from_utf8_lossy(before).into_owned())
}
