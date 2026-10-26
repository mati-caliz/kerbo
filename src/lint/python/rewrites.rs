use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, CONCATENATED_STRING, FALSE, FUNCTION_FIELD,
    IDENTIFIER, KEYWORD_ARGUMENT, LIST, NAME_FIELD, OBJECT_FIELD, STRING, TRUE, TUPLE, VALUE_FIELD, is_bytes,
    is_f_string, is_identifier, named, string_prefix, text, unparenthesized,
};
use super::codegen;
use super::escapes::text_value;
use super::fix_edits::{Parentheses, commented_unsafe, pad, remove_argument};
use super::semantic::preferred_quote;
use crate::fix::Fix;
use crate::rules::PythonRule;

const JOIN: &str = "join";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const STARRED: [&str; 2] = ["list_splat", "parenthesized_list_splat"];
const NUMBERS: [&str; 2] = ["integer", "float"];
const RAW_PREFIX: char = 'r';
const NEWLINES: [char; 2] = ['\n', '\r'];
const TRIPLE_QUOTES: [&str; 2] = ["'''", "\"\"\""];
const QUOTES: [char; 2] = ['\'', '"'];

// FLY002 (`" ".join([a, b])`, que es una f-string) y PIE804 (`f(**{"a": 1})`, que es `f(a=1)`).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    for call in nodes.iter().filter(|node| node.kind() == CALL) {
        if is_static_join(*call, source) {
            reports.push(Report::at(PythonRule::FLY002, *call, "").with_fix(static_join_fix(*call, source)));
        }
        let duplicates = duplicate_keywords(*call, source);
        reports.extend(
            arguments(*call)
                .into_iter()
                .filter(|argument| is_unnecessary_dict_kwargs(*argument, source))
                .map(|argument| {
                    let fix = dict_kwargs_fix(*call, argument, &duplicates, source);
                    Report::at(PythonRule::PIE804, argument, "").with_fix(fix)
                }),
        );
    }
    reports
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect()
}

fn plain_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    let node = unparenthesized(node);
    let parts = if node.kind() == CONCATENATED_STRING {
        named(node)
    } else {
        vec![node]
    };
    let plain = matches!(node.kind(), STRING | CONCATENATED_STRING)
        && parts
            .iter()
            .all(|part| !is_f_string(*part, source) && !is_bytes(*part, source));
    if plain { text_value(node, source) } else { None }
}

// El arreglo de FLY002: un string si todo son strings, o una f-string con cada elemento entre llaves
// y el separador en el medio; inseguro, como en ruff. Sin arreglo cuando habría que escapar algo más
// que las llaves.
fn static_join_fix(call: Node<'_>, source: &[u8]) -> Option<Fix> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let joiner_node = function.child_by_field_name(OBJECT_FIELD)?;
    let joiner = plain_string(joiner_node, source)?;
    let sequence = unparenthesized(*arguments(call).first()?);
    let elements: Vec<Node<'_>> = named(sequence)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect();
    let strings: Option<Vec<String>> = elements
        .iter()
        .map(|element| plain_string(*element, source))
        .collect();
    let contents = if let Some(strings) = strings {
        let first = unparenthesized(*elements.first()?);
        let first = if first.kind() == CONCATENATED_STRING {
            *named(first).first()?
        } else {
            first
        };
        let prefix = string_prefix(first, source);
        let quote = text(first, source).get(prefix.len()..)?.chars().next()?;
        let merged = strings.join(&joiner);
        if needs_escaping(&merged, quote) || prefix.contains(RAW_PREFIX) {
            return None;
        }
        format!("{prefix}{quote}{merged}{quote}")
    } else {
        let mut parts = Vec::new();
        for (index, element) in elements.iter().enumerate() {
            if index > 0 {
                parts.push(escaped_braces(&joiner));
            }
            parts.push(plain_string(*element, source).map_or_else(
                || format!("{{{}}}", codegen::unparse(*element, 0, source)),
                |literal| escaped_braces(&literal),
            ));
        }
        let body: String = parts.concat();
        let quote = preferred_quote(call, source);
        let has_single = body.contains('\'') || joiner.contains('\'');
        let has_double = body.contains('"') || joiner.contains('"');
        let quote = match (quote, has_single, has_double) {
            (_, true, true) => return None,
            ('\'', true, false) => '"',
            ('"', false, true) => '\'',
            (quote, _, _) => quote,
        };
        if needs_escaping(&body, quote) {
            return None;
        }
        format!("f{quote}{body}{quote}")
    };
    Some(Fix::replace(call, &pad(contents, call, source), source).marked_unsafe())
}

fn needs_escaping(content: &str, quote: char) -> bool {
    content.contains(['\\', '\n', '\r', quote])
}

fn escaped_braces(literal: &str) -> String {
    literal.replace('{', "{{").replace('}', "}}")
}

fn is_static_join(call: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)
    else {
        return false;
    };
    let is_join = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == JOIN);
    let Some(joiner) = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|_| is_join)
        .and_then(|object| plain_string(object, source))
    else {
        return false;
    };
    let given = arguments(call);
    let [sequence] = given.as_slice() else {
        return false;
    };
    let sequence = unparenthesized(*sequence);
    if !matches!(sequence.kind(), LIST | TUPLE) {
        return false;
    }
    let elements: Vec<Node<'_>> = named(sequence)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect();
    if elements.iter().any(|element| STARRED.contains(&element.kind())) {
        return false;
    }
    builds_fstring(&joiner, &elements, source)
}

// `build_fstring` de flynt: todo strings se juntan en uno; si no, cada elemento tiene que poder ir
// en una f-string y las comillas tienen que alcanzar.
fn builds_fstring(joiner: &str, elements: &[Node<'_>], source: &[u8]) -> bool {
    let strings: Option<Vec<String>> = elements
        .iter()
        .map(|element| plain_string(*element, source))
        .collect();
    if let Some(strings) = strings {
        let Some(first) = elements.first() else {
            return false;
        };
        let content = strings.join(joiner);
        let raw = string_prefix(first_part(*first), source)
            .to_ascii_lowercase()
            .contains(RAW_PREFIX);
        let needs_triple = raw && content.contains(NEWLINES);
        return !(needs_triple && TRIPLE_QUOTES.iter().all(|quotes| content.contains(quotes)));
    }
    let mut quotes_used: Vec<char> = QUOTES
        .iter()
        .copied()
        .filter(|quote| joiner.contains(*quote))
        .collect();
    for element in elements {
        let element = unparenthesized(*element);
        if let Some(literal) = plain_string(element, source) {
            quotes_used.extend(QUOTES.iter().copied().filter(|quote| literal.contains(*quote)));
        } else if !is_interpolable(element) {
            return false;
        }
    }
    !QUOTES.iter().all(|quote| quotes_used.contains(quote))
}

fn first_part(string: Node<'_>) -> Node<'_> {
    let string = unparenthesized(string);
    if string.kind() == CONCATENATED_STRING {
        named(string).first().copied().unwrap_or(string)
    } else {
        string
    }
}

// `to_interpolated_string_element`: números, booleanos, nombres, atributos y llamadas sin argumentos.
fn is_interpolable(element: Node<'_>) -> bool {
    match element.kind() {
        TRUE | FALSE | IDENTIFIER | ATTRIBUTE => true,
        CALL => {
            arguments(element).is_empty()
                && element
                    .child_by_field_name(FUNCTION_FIELD)
                    .is_some_and(is_simple_callee)
        }
        kind => NUMBERS.contains(&kind),
    }
}

fn is_simple_callee(callee: Node<'_>) -> bool {
    match callee.kind() {
        IDENTIFIER => true,
        ATTRIBUTE => callee
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(is_simple_callee),
        _ => false,
    }
}

// El arreglo de PIE804: `**{**x}` pasa a `**x`, `**{}` se borra y `**{"a": 1}` pasa a `a=1` si ningún
// nombre se repite en la llamada (inseguro si el diccionario tiene comentarios).
fn dict_kwargs_fix(call: Node<'_>, splat: Node<'_>, duplicates: &[String], source: &[u8]) -> Option<Fix> {
    let dictionary = named(splat).first().map(|value| unparenthesized(*value))?;
    let items: Vec<Node<'_>> = named(dictionary)
        .into_iter()
        .filter(|item| item.kind() != COMMENT)
        .collect();
    if let [only] = items.as_slice()
        && only.kind() == DICTIONARY_SPLAT
    {
        let value = named(*only).first().map_or("", |value| text(*value, source));
        return Some(Fix::replace(splat, &format!("**{value}"), source));
    }
    if items.is_empty() {
        let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
        return Some(remove_argument(splat, arguments, Parentheses::Preserve, source));
    }
    let keywords: Vec<(String, &str)> = items
        .iter()
        .filter_map(|pair| {
            let key = pair
                .child_by_field_name(KEY_FIELD)
                .and_then(|key| plain_string(key, source))?;
            let value = pair.child_by_field_name(VALUE_FIELD)?;
            Some((key, text(value, source)))
        })
        .collect();
    if keywords.iter().any(|(key, _)| duplicates.contains(key)) {
        return None;
    }
    let replacement = keywords
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join(", ");
    Some(commented_unsafe(
        Fix::replace(splat, &replacement, source),
        dictionary,
        source,
    ))
}

// Los nombres que aparecen más de una vez entre los argumentos con nombre y las claves de los `**{...}`.
fn duplicate_keywords(call: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut duplicates = Vec::new();
    for argument in arguments(call) {
        let names: Vec<String> = if argument.kind() == KEYWORD_ARGUMENT {
            argument
                .child_by_field_name(NAME_FIELD)
                .map(|name| text(name, source).to_owned())
                .into_iter()
                .collect()
        } else if argument.kind() == DICTIONARY_SPLAT {
            named(argument)
                .first()
                .map(|value| unparenthesized(*value))
                .filter(|value| value.kind() == DICTIONARY)
                .map(named)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
                .filter_map(|key| plain_string(key, source))
                .filter(|key| is_identifier(key))
                .collect()
        } else {
            Vec::new()
        };
        for name in names {
            if seen.contains(&name) {
                duplicates.push(name);
            } else {
                seen.push(name);
            }
        }
    }
    duplicates
}

fn is_unnecessary_dict_kwargs(argument: Node<'_>, source: &[u8]) -> bool {
    if argument.kind() != DICTIONARY_SPLAT {
        return false;
    }
    let Some(dictionary) = named(argument).first().map(|value| unparenthesized(*value)) else {
        return false;
    };
    if dictionary.kind() != DICTIONARY {
        return false;
    }
    let items: Vec<Node<'_>> = named(dictionary)
        .into_iter()
        .filter(|item| item.kind() != COMMENT)
        .collect();
    if matches!(items.as_slice(), [only] if only.kind() == DICTIONARY_SPLAT) {
        return true;
    }
    items.iter().all(|item| {
        item.kind() == PAIR
            && item
                .child_by_field_name(KEY_FIELD)
                .and_then(|key| plain_string(key, source))
                .is_some_and(|key| is_identifier(&key))
    })
}
