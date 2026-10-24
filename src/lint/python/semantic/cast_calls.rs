use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::model::Model;
use super::qualified::{is_typing, qualified_name};
use super::type_hints::Hints;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, NAME_FIELD, RIGHT_FIELD,
    SUBSCRIPT, SUBSCRIPT_FIELD, TRUE, TUPLE, VALUE_FIELD, is_bytes, is_string, named, text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const BUILTINS_PREFIX: &str = "builtins.";
const LIST_COMPREHENSION: &str = "list_comprehension";
const DICTIONARY: &str = "dictionary";
const DICTIONARY_COMPREHENSION: &str = "dictionary_comprehension";
const SLICE: &str = "slice";
const INTEGER: &str = "integer";
const BODY_FIELD: &str = "body";
const GETENV: [&str; 2] = ["os.getenv", "os.environ.get"];
const DEFAULT: &str = "default";
const NON_STRING_DEFAULTS: [&str; 10] = [
    INTEGER,
    "float",
    TRUE,
    "false",
    "list",
    TUPLE,
    DICTIONARY,
    "set",
    LIST_COMPREHENSION,
    DICTIONARY_COMPREHENSION,
];
const ZIP: &str = "zip";
const TYPE_VARIABLES: [&str; 2] = ["TypeVar", "ParamSpec"];
const COVARIANT: &str = "covariant";
const CONTRAVARIANT: &str = "contravariant";
const INFER_VARIANCE: &str = "infer_variance";
const NAME_KEYWORD: &str = "name";
const COVARIANT_SUFFIX: &str = "_co";
const CONTRAVARIANT_SUFFIX: &str = "_contra";
const SORTED: &str = "sorted";

// Llamadas que repiten o envuelven de más: C414 (`list(tuple(x))`), C403 (`set([...])` con una
// comprensión), C404 (`dict([(k, v) for ...])`), C411 (`list([...])`), C418 (`dict({...})`), y además
// PLW1508 (`os.getenv` con un default que no es texto), RUF007 (`zip(x, x[1:])`) y PLC0105 (el nombre
// de un `TypeVar` que no dice su varianza).
pub fn cast_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let builtin = builtin_name(model, source, function);
    let mut reports = Vec::new();
    if let Some(outer) = builtin.as_deref() {
        reports.extend(collection_wrapper(Hints { model, source }, call, outer, &given));
        if outer == ZIP && is_successive_pair(&given, source) {
            reports.push(Report::at(PythonRule::RUF007, call, ""));
        }
    }
    let qualified = qualified_name(model, function, source);
    if qualified.as_deref().is_some_and(|name| GETENV.contains(&name))
        && let Some(default) =
            keyword_value(&given, DEFAULT, source).or_else(|| given.positional.get(1).copied())
        && is_non_string_default(unparenthesized(default), source)
    {
        reports.push(Report::at(PythonRule::PLW1508, unparenthesized(default), ""));
    }
    if qualified
        .as_deref()
        .is_some_and(|name| TYPE_VARIABLES.iter().any(|member| is_typing(name, member)))
        && is_assigned(call)
        && has_wrong_variance(&given, source)
    {
        reports.push(Report::at(PythonRule::PLC0105, call, ""));
    }
    reports
}

fn builtin_name(model: &Model<'_>, source: &[u8], function: Node<'_>) -> Option<String> {
    qualified_name(model, function, source)
        .and_then(|name| name.strip_prefix(BUILTINS_PREFIX).map(str::to_owned))
}

fn collection_wrapper(
    hints: Hints<'_, '_, '_>,
    call: Node<'_>,
    outer: &str,
    given: &Arguments<'_>,
) -> Option<Report> {
    let [argument] = given.positional.as_slice() else {
        return None;
    };
    let argument = unparenthesized(*argument);
    let only_positional = given.keywords.is_empty();
    let rule = match (outer, argument.kind()) {
        ("set", LIST_COMPREHENSION) if only_positional => Some(PythonRule::C403),
        ("list", LIST_COMPREHENSION) if only_positional => Some(PythonRule::C411),
        ("dict", LIST_COMPREHENSION) if only_positional && yields_pairs(argument) => Some(PythonRule::C404),
        ("dict", DICTIONARY | DICTIONARY_COMPREHENSION) if only_positional => Some(PythonRule::C418),
        (_, CALL) => double_cast(hints, outer, given, argument),
        _ => None,
    };
    rule.map(|rule| Report::at(rule, call, ""))
}

fn yields_pairs(comprehension: Node<'_>) -> bool {
    comprehension
        .child_by_field_name(BODY_FIELD)
        .map(unparenthesized)
        .is_some_and(|body| {
            body.kind() == TUPLE
                && named(body)
                    .into_iter()
                    .filter(|element| element.kind() != COMMENT)
                    .count()
                    == 2
        })
}

// `unnecessary_double_cast_or_process` de ruff.
fn double_cast(
    hints: Hints<'_, '_, '_>,
    outer: &str,
    outer_arguments: &Arguments<'_>,
    inner_call: Node<'_>,
) -> Option<PythonRule> {
    let source = hints.source;
    let inner = builtin_name(
        hints.model,
        source,
        inner_call.child_by_field_name(FUNCTION_FIELD)?,
    )?;
    let inner_arguments = arguments(inner_call);
    if inner_arguments.positional.is_empty() {
        return None;
    }
    if outer == SORTED && inner == SORTED && !same_keywords(outer_arguments, &inner_arguments, source) {
        return None;
    }
    let redundant = match outer {
        "set" | "sorted" => {
            matches!(inner.as_str(), "list" | "tuple" | "reversed" | "sorted")
                || (outer == "set" && inner == "set")
        }
        "list" | "tuple" => matches!(inner.as_str(), "list" | "tuple"),
        _ => false,
    };
    redundant.then_some(PythonRule::C414)
}

fn same_keywords(outer: &Arguments<'_>, inner: &Arguments<'_>, source: &[u8]) -> bool {
    outer.keywords.len() == inner.keywords.len()
        && outer.keywords.iter().all(|keyword| {
            inner
                .keywords
                .iter()
                .any(|other| equivalent(*keyword, *other, source))
        })
}

// `ResolvedPythonType` de ruff: algo que se sabe que no es texto ni `None`.
fn is_non_string_default(default: Node<'_>, source: &[u8]) -> bool {
    NON_STRING_DEFAULTS.contains(&default.kind()) || (is_string(default) && is_bytes(default, source))
}

// RUF007: `zip(x, x[1:])`, donde los dos lados recorren el mismo nombre corrido en uno.
fn is_successive_pair(given: &Arguments<'_>, source: &[u8]) -> bool {
    let [first, second] = given.positional.as_slice() else {
        return false;
    };
    let second = unparenthesized(*second);
    if second.kind() != SUBSCRIPT {
        return false;
    }
    match (
        slice_start(unparenthesized(*first), source),
        slice_start(second, source),
    ) {
        (Some((first_name, first_start)), Some((second_name, second_start))) => {
            first_name == second_name && second_start == first_start.saturating_add(1)
        }
        _ => false,
    }
}

// El nombre y el comienzo de `x` (0) o de `x[n:]`.
fn slice_start<'source>(node: Node<'_>, source: &'source [u8]) -> Option<(&'source str, u64)> {
    if node.kind() == IDENTIFIER {
        return Some((text(node, source), 0));
    }
    let value = node
        .child_by_field_name(VALUE_FIELD)
        .filter(|value| node.kind() == SUBSCRIPT && value.kind() == IDENTIFIER)?;
    let mut cursor = node.walk();
    let slices: Vec<Node<'_>> = node
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    let [slice] = slices.as_slice() else {
        return None;
    };
    if slice.kind() != SLICE {
        return None;
    }
    let parts: Vec<Node<'_>> = named(*slice)
        .into_iter()
        .filter(|part| part.kind() != COMMENT)
        .collect();
    let [start] = parts.as_slice() else {
        return None;
    };
    let colons = text(*slice, source).matches(':').count();
    let starts_slice = slice.start_byte() == start.start_byte();
    if start.kind() != INTEGER || colons != 1 || !starts_slice {
        return None;
    }
    let start = text(*start, source).parse().ok()?;
    Some((text(value, source), start))
}

fn is_assigned(call: Node<'_>) -> bool {
    call.parent().is_some_and(|parent| {
        parent.kind() == ASSIGNMENT
            && parent
                .child_by_field_name(RIGHT_FIELD)
                .is_some_and(|value| value.id() == call.id())
    })
}

// PLC0105: `TypeVar("T", covariant=True)` sin `_co`, o un nombre con `_co` invariante.
fn has_wrong_variance(given: &Arguments<'_>, source: &[u8]) -> bool {
    let Some(name) = given
        .positional
        .first()
        .copied()
        .or_else(|| keyword_value(given, NAME_KEYWORD, source))
        .filter(|first| is_string(*first))
        .and_then(|first| text_value(first, source))
    else {
        return false;
    };
    let is_set = |keyword: &str| {
        given
            .keywords
            .iter()
            .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
            .any(|argument| {
                argument
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|key| text(key, source) == keyword)
                    && argument
                        .child_by_field_name(VALUE_FIELD)
                        .is_some_and(|value| unparenthesized(value).kind() == TRUE)
            })
    };
    if is_set(INFER_VARIANCE) {
        return false;
    }
    match (is_set(COVARIANT), is_set(CONTRAVARIANT)) {
        (true, true) => false,
        (true, false) => !name.ends_with(COVARIANT_SUFFIX),
        (false, true) => !name.ends_with(CONTRAVARIANT_SUFFIX),
        (false, false) => name.ends_with(COVARIANT_SUFFIX) || name.ends_with(CONTRAVARIANT_SUFFIX),
    }
}
