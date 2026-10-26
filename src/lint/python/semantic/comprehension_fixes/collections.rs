use tree_sitter::Node;

use super::{LIST, edit, in_interpolation, padded_close, padded_open, unsafe_fix};
use crate::fix::Fix;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, COMMENT, KEYWORD_ARGUMENT, NAME_FIELD, STRING, VALUE_FIELD, named, string_prefix, text,
};

const DICT: &str = "dict";
const DOUBLE_QUOTE: char = '"';
const SINGLE_QUOTE: char = '\'';
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const OPEN_BRACE: u8 = b'{';
const LIST_KIND: &str = "list";
const TUPLE_KIND: &str = "tuple";
const GENERATOR_KIND: &str = "generator_expression";
const NOT_OPERATOR: &str = "not_operator";
const TRUE_KIND: &str = "true";
const FALSE_KIND: &str = "false";
const TRUE: &str = "True";
const FALSE: &str = "False";
const REVERSE: &str = "reverse";
const IDENTIFIER: &str = "identifier";
const LINE_FEED: u8 = b'\n';
const FUNCTION_FIELD: &str = "function";
const ARGUMENT_FIELD: &str = "argument";
const CLOSE_BRACE: u8 = b'}';

// C408: `dict()`, `list()` y `tuple()` pasan a `{}`, `[]` y `()`; `dict(a=1)` pasa a `{"a": 1}` con la
// comilla preferida del archivo, conservando lo demás como libcst. Inseguro, como en ruff.
pub fn collection_call_fix(call: Node<'_>, collection: &str, source: &[u8]) -> Option<Fix> {
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let keywords: Vec<Node<'_>> = named(arguments)
        .into_iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .collect();
    let inner_start = arguments.start_byte().saturating_add(1);
    let inner_end = arguments.end_byte().saturating_sub(1);
    if keywords.is_empty() {
        let (open, close) = match collection {
            DICT => (padded_open("{", call, source), padded_close("}", call, source)),
            LIST => (String::from("["), String::from("]")),
            _ => (String::from("("), String::from(")")),
        };
        return Some(unsafe_fix(
            vec![
                edit(call.start_byte(), inner_start, &open, source),
                edit(inner_end, call.end_byte(), &close, source),
            ],
            source,
        ));
    }
    let quote = preferred_quote(call, source);
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let mut dictionary = String::from("{");
    let mut previous = inner_start;
    for keyword in keywords {
        let name = keyword.child_by_field_name(NAME_FIELD)?;
        let value = keyword.child_by_field_name(VALUE_FIELD)?;
        dictionary.push_str(&slice(previous, keyword.start_byte()));
        dictionary.push(quote);
        dictionary.push_str(text(name, source));
        dictionary.push(quote);
        dictionary.push_str(": ");
        previous = value.start_byte();
    }
    dictionary.push_str(&slice(previous, inner_end));
    dictionary.push('}');
    let interpolated = in_interpolation(call);
    let before = call
        .start_byte()
        .checked_sub(1)
        .and_then(|index| source.get(index));
    if interpolated && before == Some(&OPEN_BRACE) {
        dictionary.insert(0, ' ');
    }
    if interpolated && source.get(call.end_byte()) == Some(&CLOSE_BRACE) {
        dictionary.push(' ');
    }
    Some(unsafe_fix(
        vec![edit(call.start_byte(), call.end_byte(), &dictionary, source)],
        source,
    ))
}

// La comilla de ruff: adentro de un f-string, la otra; si no, la del primer string del archivo que no
// es triple, o la doble.
pub fn preferred_quote(node: Node<'_>, source: &[u8]) -> char {
    if let Some(string) =
        std::iter::successors(node.parent(), Node::parent).find(|ancestor| ancestor.kind() == STRING)
    {
        return if quote_of(string, source) == Some(DOUBLE_QUOTE) {
            SINGLE_QUOTE
        } else {
            DOUBLE_QUOTE
        };
    }
    let root = std::iter::successors(Some(node), Node::parent)
        .last()
        .unwrap_or(node);
    first_quote(root, source).unwrap_or(DOUBLE_QUOTE)
}

fn first_quote(node: Node<'_>, source: &[u8]) -> Option<char> {
    if node.kind() == STRING {
        let literal = text(node, source)
            .get(string_prefix(node, source).len()..)
            .unwrap_or_default();
        if !TRIPLE_QUOTES.iter().any(|triple| literal.starts_with(triple)) {
            return literal.chars().next();
        }
    }
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .find_map(|child| first_quote(child, source))
}

fn quote_of(string: Node<'_>, source: &[u8]) -> Option<char> {
    text(string, source)
        .get(string_prefix(string, source).len()..)?
        .chars()
        .next()
}

// C406: `dict([(1, 2), (3, 4)])` pasa a `{1: 2, 3: 4}`, como libcst: se van los paréntesis de las
// tuplas y los espacios pegados a los corchetes; quedan las comas y lo que separa los elementos.
pub fn literal_dict_pairs_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Option<Fix> {
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    if !matches!(argument.kind(), LIST_KIND | TUPLE_KIND) {
        return None;
    }
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let pairs = significant(argument);
    let mut dictionary = format!(
        "{{{}",
        slice(arguments.start_byte().saturating_add(1), argument.start_byte())
    );
    for (index, pair) in pairs.iter().enumerate() {
        let parts = significant(*pair);
        let [key, value] = parts.as_slice() else {
            return None;
        };
        dictionary.push_str(text(*key, source));
        dictionary.push_str(": ");
        dictionary.push_str(text(*value, source));
        let next = pairs
            .get(index.saturating_add(1))
            .map_or_else(|| argument.end_byte().saturating_sub(1), Node::start_byte);
        let between = slice(pair.end_byte(), next);
        if pairs.get(index.saturating_add(1)).is_some() {
            dictionary.push_str(&between);
        } else if let Some(comma) = between.find(',') {
            dictionary.push_str(between.get(..=comma).unwrap_or_default());
        }
    }
    dictionary.push_str(&slice(
        argument.end_byte(),
        arguments.end_byte().saturating_sub(1),
    ));
    dictionary.push('}');
    Some(unsafe_fix(
        vec![edit(call.start_byte(), call.end_byte(), &dictionary, source)],
        source,
    ))
}

// C414: `sorted(list(x))` pasa a `sorted(x)`.
pub fn double_cast_fix(inner: Node<'_>, source: &[u8]) -> Option<Fix> {
    let arguments = inner.child_by_field_name(ARGUMENTS_FIELD)?;
    let first = significant(arguments)
        .into_iter()
        .find(|argument| argument.kind() != KEYWORD_ARGUMENT)?;
    Some(unsafe_fix(
        vec![edit(
            inner.start_byte(),
            inner.end_byte(),
            text(first, source),
            source,
        )],
        source,
    ))
}

// C413: `list(sorted(x))` pasa a `sorted(x)`; `reversed(sorted(x))` invierte el `reverse=` o suma
// `reverse=True` (inseguro). La negación es la de libcst: sin paréntesis. Como ruff, sólo con un
// nombre suelto afuera: con `builtins.list(...)` no cambia nada.
pub fn call_around_sorted_fix(
    outer: Node<'_>,
    inner: Node<'_>,
    reversed: bool,
    source: &[u8],
) -> Option<Fix> {
    let outer_is_name = outer
        .child_by_field_name(FUNCTION_FIELD)
        .is_some_and(|function| function.kind() == IDENTIFIER);
    if !outer_is_name {
        return None;
    }
    let function = inner.child_by_field_name(FUNCTION_FIELD)?;
    let arguments = inner.child_by_field_name(ARGUMENTS_FIELD)?;
    if source
        .get(function.end_byte()..arguments.start_byte())
        .unwrap_or_default()
        .contains(&LINE_FEED)
    {
        return None;
    }
    if !reversed {
        let fix = edit(outer.start_byte(), outer.end_byte(), text(inner, source), source);
        return Some(Fix::from_edits(source, vec![fix]));
    }
    let replacement = if arguments.kind() == GENERATOR_KIND {
        format!(
            "{}({}, {REVERSE}={TRUE})",
            text(function, source),
            text(arguments, source)
        )
    } else if let Some(keyword) = significant(arguments).into_iter().find(|argument| {
        argument.kind() == KEYWORD_ARGUMENT
            && argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == REVERSE)
    }) {
        let value = keyword.child_by_field_name(VALUE_FIELD)?;
        format!(
            "{}{}{}",
            String::from_utf8_lossy(
                source
                    .get(inner.start_byte()..value.start_byte())
                    .unwrap_or_default()
            ),
            negated(value, source),
            String::from_utf8_lossy(source.get(value.end_byte()..inner.end_byte()).unwrap_or_default()),
        )
    } else {
        let last = significant(arguments).last().copied()?;
        let closing = arguments.end_byte().saturating_sub(1);
        let after_last =
            String::from_utf8_lossy(source.get(last.end_byte()..closing).unwrap_or_default()).into_owned();
        let separator = if after_last.contains(',') { "" } else { ", " };
        format!(
            "{}{separator}{after_last}{REVERSE}={TRUE})",
            String::from_utf8_lossy(
                source
                    .get(inner.start_byte()..last.end_byte())
                    .unwrap_or_default()
            ),
        )
    };
    Some(unsafe_fix(
        vec![edit(outer.start_byte(), outer.end_byte(), &replacement, source)],
        source,
    ))
}

fn negated(value: Node<'_>, source: &[u8]) -> String {
    match value.kind() {
        NOT_OPERATOR => value.child_by_field_name(ARGUMENT_FIELD).map_or_else(
            || text(value, source).to_owned(),
            |operand| text(operand, source).to_owned(),
        ),
        TRUE_KIND => String::from(FALSE),
        FALSE_KIND => String::from(TRUE),
        _ => format!("not {}", text(value, source)),
    }
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}
