use std::collections::HashSet;

use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::cast_calls::is_non_string_default;
use super::handlers::is_logger_candidate;
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, NAME_FIELD, NONE, OBJECT_FIELD, STRING,
    is_bytes, is_f_string, is_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::{part_value, text_value};
use crate::lint::python::formats::percent::summary;
use crate::lint::python::known_types::{Known, is_known};
use crate::rules::PythonRule;

const DICTIONARY: &str = "dictionary";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const LIST_SPLAT: &str = "list_splat";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const LOGGING_PREFIX: &str = "logging.";
const LOGGING_LEVELS: [&str; 7] = [
    "debug",
    "critical",
    "error",
    "exception",
    "info",
    "warn",
    "warning",
];
const STRIP_METHODS: [&str; 3] = ["strip", "lstrip", "rstrip"];
const GETENV: &str = "os.getenv";
const KEY: &str = "key";
const SHALLOW_COPY: &str = "copy.copy";
const ENVIRON: &str = "os.environ";
const OPEN: &str = "open";
const PATH: &str = "pathlib.Path";
const MODE: &str = "mode";
const LOCKS: [&str; 5] = [
    "threading.Lock",
    "threading.RLock",
    "threading.Condition",
    "threading.Semaphore",
    "threading.BoundedSemaphore",
];
const WITH_ITEM: &str = "with_item";
const AS_PATTERN: &str = "as_pattern";
const BUILTIN_MODE_POSITION: usize = 1;
const MODE_FLAGS: &str = "rwxabt+U";
const PRIMARY_MODES: [char; 4] = ['r', 'w', 'x', 'a'];
const WRITING_MODES: [char; 4] = ['w', 'x', 'a', '+'];
const TEXT_MODE: char = 't';
const BINARY_MODE: char = 'b';
const UNIVERSAL_NEWLINES: char = 'U';

// Reglas de pylint sobre una llamada: PLE1132 (clave repetida entre `k=` y `**{"k": ...}`), PLE1205 y
// PLE1206 (cantidad de argumentos de un logging con `%`), PLE1310 (`strip` con caracteres repetidos),
// PLE1507 (`os.getenv` con una clave que no es texto), PLW1501 (modo inválido de `open`) y PLW1507
// (`copy.copy(os.environ)`).
pub fn pylint_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let mut reports = repeated_keywords(&given, source);
    reports.extend(logging_arguments(model, source, function, &given));
    reports.extend(duplicated_strip_characters(source, function, &given));
    reports.extend(invalid_open_mode(model, source, function, &given));
    if is_lock(model, source, function) && is_useless_with_item(model, source, call) {
        reports.push(Report::at(PythonRule::PLW2101, call, ""));
    }
    match qualified_name(model, function, source).as_deref() {
        Some(GETENV) => {
            let key = keyword_value(&given, KEY, source).or_else(|| given.positional.first().copied());
            reports.extend(
                key.map(unparenthesized)
                    .filter(|key| key.kind() == NONE || is_non_string_default(*key, source))
                    .map(|key| Report::at(PythonRule::PLE1507, key, "")),
            );
        }
        Some(SHALLOW_COPY) => {
            let copies_environ = given.keywords.is_empty()
                && matches!(given.positional.as_slice(), [only]
                    if qualified_name(model, *only, source).as_deref() == Some(ENVIRON));
            if copies_environ {
                reports.push(Report::at(PythonRule::PLW1507, call, ""));
            }
        }
        _ => {}
    }
    reports
}

// PLE1132: un nombre que ya está como `k=` (o en otro `**{...}`) y se repite como clave de texto de un
// `**{...}` literal. Avisa en la clave repetida.
fn repeated_keywords(given: &Arguments<'_>, source: &[u8]) -> Vec<Report> {
    let mut seen: HashSet<String> = given
        .keywords
        .iter()
        .filter_map(|keyword| keyword.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source).to_owned())
        .collect();
    let mut reports = Vec::new();
    let splatted = given
        .keywords
        .iter()
        .filter(|keyword| keyword.kind() == DICTIONARY_SPLAT)
        .filter_map(|splat| named(*splat).first().copied())
        .map(unparenthesized)
        .filter(|dictionary| dictionary.kind() == DICTIONARY);
    for dictionary in splatted {
        let keys = named(dictionary)
            .into_iter()
            .filter(|entry| entry.kind() == PAIR)
            .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
            .filter(|key| is_string(*key) && !is_f_string(*key, source) && !is_bytes(*key, source));
        for key in keys {
            if let Some(value) = text_value(key, source)
                && !seen.insert(value)
            {
                reports.push(Report::at(PythonRule::PLE1132, key, ""));
            }
        }
    }
    reports
}

// PLE1205 y PLE1206: un `logging.info("%s %s", x)` con más o menos argumentos que `%s`. Avisa en la
// función llamada.
fn logging_arguments(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    given: &Arguments<'_>,
) -> Option<Report> {
    let starred = given
        .positional
        .iter()
        .any(|argument| argument.kind() == LIST_SPLAT)
        || given
            .keywords
            .iter()
            .any(|keyword| keyword.kind() == DICTIONARY_SPLAT);
    if starred || !is_logging_call(model, source, function) {
        return None;
    }
    let format = given
        .positional
        .first()
        .copied()
        .filter(|first| is_string(*first) && !is_f_string(*first, source) && !is_bytes(*first, source))?;
    let summary = summary(&text_value(format, source)?).ok()?;
    if summary.starred || !summary.keywords.is_empty() {
        return None;
    }
    let message_arguments = given.positional.len().saturating_sub(1);
    if summary.positional < message_arguments {
        return Some(Report::at(PythonRule::PLE1205, function, ""));
    }
    (message_arguments > 0 && given.keywords.is_empty() && summary.positional > message_arguments)
        .then(|| Report::at(PythonRule::PLE1206, function, ""))
}

fn is_logging_call(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    match function.kind() {
        ATTRIBUTE => {
            function
                .child_by_field_name(ATTRIBUTE_FIELD)
                .is_some_and(|level| LOGGING_LEVELS.contains(&text(level, source)))
                && is_logger_candidate(model, source, function)
        }
        IDENTIFIER => qualified_name(model, function, source).is_some_and(|name| {
            name.strip_prefix(LOGGING_PREFIX)
                .is_some_and(|level| LOGGING_LEVELS.contains(&level))
        }),
        _ => false,
    }
}

// PLE1310: `x.strip("aa")`, con caracteres repetidos en el argumento (que no es un prefijo). Avisa en
// el argumento.
fn duplicated_strip_characters(source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> Option<Report> {
    if function.kind() != ATTRIBUTE || !given.keywords.is_empty() {
        return None;
    }
    let method = text(function.child_by_field_name(ATTRIBUTE_FIELD)?, source);
    let [argument] = given.positional.as_slice() else {
        return None;
    };
    if !STRIP_METHODS.contains(&method) {
        return None;
    }
    let object = unparenthesized(function.child_by_field_name(OBJECT_FIELD)?);
    let kind = if is_string(object) && !is_f_string(object, source) {
        if is_bytes(object, source) {
            Known::Bytes
        } else {
            Known::String
        }
    } else if is_known(object, Known::String, source) {
        Known::String
    } else if is_known(object, Known::Bytes, source) {
        Known::Bytes
    } else {
        return None;
    };
    let argument = *argument;
    let matches_kind = is_string(argument)
        && !is_f_string(argument, source)
        && is_bytes(argument, source) == (kind == Known::Bytes);
    if !matches_kind {
        return None;
    }
    let characters: String = if argument.kind() == STRING {
        part_value(argument, source)
    } else {
        named(argument)
            .into_iter()
            .map(|part| part_value(part, source))
            .collect()
    };
    let mut seen = HashSet::new();
    characters
        .chars()
        .any(|character| !seen.insert(character))
        .then(|| Report::at(PythonRule::PLE1310, argument, ""))
}

// PLW1501: un modo de `open` (o de `pathlib.Path(...).open`) que Python rechaza. Avisa en el modo.
fn invalid_open_mode(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    given: &Arguments<'_>,
) -> Option<Report> {
    let position = if is_builtin_named(model, function, source, OPEN) {
        BUILTIN_MODE_POSITION
    } else if is_path_open(model, source, function) {
        0
    } else {
        return None;
    };
    let mode = keyword_value(given, MODE, source).or_else(|| given.positional.get(position).copied())?;
    if !is_string(mode) || is_f_string(mode, source) || is_bytes(mode, source) {
        return None;
    }
    let value = text_value(mode, source)?;
    (!is_valid_open_mode(&value)).then(|| Report::at(PythonRule::PLW1501, mode, ""))
}

fn is_path_open(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == OPEN)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| object.kind() == CALL)
            .and_then(|object| object.child_by_field_name(FUNCTION_FIELD))
            .and_then(|factory| qualified_name(model, factory, source))
            .is_some_and(|name| name == PATH)
}

// `OpenMode::from_chars`: cada letra una sola vez, no texto y binario juntos, `U` sólo para leer y
// exactamente uno de leer, escribir, crear o agregar (o sólo `U`).
fn is_valid_open_mode(mode: &str) -> bool {
    let mut seen = HashSet::new();
    if !mode
        .chars()
        .all(|flag| MODE_FLAGS.contains(flag) && seen.insert(flag))
    {
        return false;
    }
    let has = |flag: &char| seen.contains(flag);
    let primary = PRIMARY_MODES.iter().filter(|flag| has(flag)).count();
    let universal = has(&UNIVERSAL_NEWLINES);
    if (has(&TEXT_MODE) && has(&BINARY_MODE)) || (universal && WRITING_MODES.iter().any(has)) {
        return false;
    }
    primary == 1 || (primary == 0 && universal)
}

fn is_lock(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    qualified_name(model, function, source).is_some_and(|name| LOCKS.contains(&name.as_str()))
}

// PLW2101: un candado nuevo como contexto de un `with`, que nadie más comparte. Como ruff, se corta en
// el primer elemento del `with` que es una llamada a otra cosa.
fn is_useless_with_item(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    let holder = call
        .parent()
        .filter(|parent| parent.kind() == AS_PATTERN)
        .unwrap_or(call);
    let Some(item) = holder.parent().filter(|parent| parent.kind() == WITH_ITEM) else {
        return false;
    };
    let Some(clause) = item.parent() else {
        return false;
    };
    named(clause)
        .into_iter()
        .take_while(|other| other.id() != item.id())
        .filter_map(|other| named(other).first().copied())
        .map(|value| {
            if value.kind() == AS_PATTERN {
                named(value).first().copied().unwrap_or(value)
            } else {
                value
            }
        })
        .filter(|value| value.kind() == CALL)
        .all(|value| {
            value
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| is_lock(model, source, function))
        })
}
