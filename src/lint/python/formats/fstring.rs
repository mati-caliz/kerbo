use tree_sitter::Node;

use super::braces;
use super::template::{self, FieldType};
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, COMMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, NAME_FIELD,
    OBJECT_FIELD, STRING, VALUE_FIELD, is_string, named, string_prefix, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;

const FORMAT: &str = "format";
const GENERATOR: &str = "generator_expression";
const SPLATS: [&str; 2] = ["list_splat", "dictionary_splat"];
const QUOTES: [char; 2] = ['\'', '"'];
const LINE_BREAKS: [char; 2] = ['\n', '\r'];
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const TRIPLE_QUOTE_LENGTH: usize = 3;
const QUOTE_LENGTH: usize = 1;
const UNICODE_PREFIXES: [char; 2] = ['u', 'U'];
const RAW_PREFIXES: [char; 2] = ['r', 'R'];
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];

#[derive(Debug, Default)]
struct Arguments<'tree, 'source> {
    positional: Vec<Node<'tree>>,
    keywords: Vec<(&'source str, Node<'tree>)>,
}

impl<'tree> Arguments<'tree, '_> {
    fn find(&self, field: &FieldType, next_auto: &mut usize) -> Option<(Specifier, Node<'tree>)> {
        match field {
            FieldType::Auto => {
                let index = *next_auto;
                *next_auto = next_auto.saturating_add(1);
                Some((Specifier::Index(index), *self.positional.get(index)?))
            }
            FieldType::Index(index) => Some((Specifier::Index(*index), *self.positional.get(*index)?)),
            FieldType::Keyword(name) => self
                .keywords
                .iter()
                .rev()
                .find(|(keyword, _)| keyword == name)
                .map(|(_, value)| (Specifier::Keyword(name.clone()), *value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Specifier {
    Index(usize),
    Keyword(String),
}

// UP032, la parte sintáctica: un `"…".format(…)` que se puede escribir como f-string. Calca
// `f_strings` de ruff: argumentos sin `*`/`**`, sin comillas ni saltos de línea, y cada parte del
// string con un formato válido cuyos campos apuntan a argumentos que existen y que no se repiten si
// tienen efectos. `has_effect` es el `contains_effect` de ruff sin tratar distinto a los builtins.
pub fn is_convertible_format_call(
    call: Node<'_>,
    source: &[u8],
    has_effect: &dyn Fn(Node<'_>) -> bool,
) -> bool {
    let Some(object) = format_object(call, source) else {
        return false;
    };
    let has_nested_parts = text_value(object, source)
        .and_then(|format| braces::summary(&format))
        .is_none_or(|summary| summary.has_nested_parts);
    if has_nested_parts {
        return false;
    }
    let Some(arguments) = arguments(call, source) else {
        return false;
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
    segments
        .iter()
        .all(|segment| is_convertible_segment(text(*segment, source), &arguments, &mut next_auto, has_effect))
}

fn format_object<'tree>(call: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .filter(|attribute| text(*attribute, source) == FORMAT)?;
    function
        .child_by_field_name(OBJECT_FIELD)
        .map(unparenthesized)
        .filter(|object| is_string(*object) && !is_template(*object, source))
}

fn is_template(object: Node<'_>, source: &[u8]) -> bool {
    let parts = if object.kind() == STRING {
        vec![object]
    } else {
        named(object)
    };
    parts
        .iter()
        .any(|part| part.kind() == STRING && string_prefix(*part, source).contains(TEMPLATE_PREFIXES))
}

// `FormatSummaryValues::try_from_call`.
fn arguments<'tree, 'source>(call: Node<'tree>, source: &'source [u8]) -> Option<Arguments<'tree, 'source>> {
    let list = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let items = if list.kind() == GENERATOR {
        vec![list]
    } else {
        named(list)
    };
    let mut arguments = Arguments::default();
    for item in items.into_iter().filter(|item| item.kind() != COMMENT) {
        if SPLATS.contains(&item.kind()) {
            return None;
        }
        if item.kind() == KEYWORD_ARGUMENT {
            let name = text(item.child_by_field_name(NAME_FIELD)?, source);
            let value = item.child_by_field_name(VALUE_FIELD)?;
            arguments.keywords.push((name, simple_value(value, source)?));
        } else {
            arguments.positional.push(simple_value(item, source)?);
        }
    }
    (!arguments.positional.is_empty() || !arguments.keywords.is_empty()).then_some(arguments)
}

fn simple_value<'tree>(value: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let value = unparenthesized(value);
    let value_text = text(value, source);
    (!value_text.contains(QUOTES) && !value_text.contains(LINE_BREAKS)).then_some(value)
}

// `FStringConversion::try_convert` sobre una parte del string, con su prefijo y sus comillas.
fn is_convertible_segment(
    segment: &str,
    arguments: &Arguments<'_, '_>,
    next_auto: &mut usize,
    has_effect: &dyn Fn(Node<'_>) -> bool,
) -> bool {
    let unprefixed = segment.strip_prefix(UNICODE_PREFIXES).unwrap_or(segment);
    let raw = unprefixed.starts_with(RAW_PREFIXES);
    let quoted = unprefixed.strip_prefix(RAW_PREFIXES).unwrap_or(unprefixed);
    let quote_length = if TRIPLE_QUOTES.iter().any(|quote| quoted.starts_with(quote)) {
        TRIPLE_QUOTE_LENGTH
    } else {
        QUOTE_LENGTH
    };
    let Some(contents) = quoted.get(quote_length..quoted.len().saturating_sub(quote_length)) else {
        return false;
    };
    let Some(replacements) = template::replacements(contents, raw) else {
        return false;
    };
    let mut seen: Vec<Specifier> = Vec::new();
    for replacement in replacements {
        let Some(field) = template::field_type(&replacement.field_name) else {
            return false;
        };
        let Some((specifier, value)) = arguments.find(&field, next_auto) else {
            return false;
        };
        if seen.contains(&specifier) && has_effect(value) {
            return false;
        }
        seen.push(specifier);
    }
    true
}
