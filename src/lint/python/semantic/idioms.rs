use tree_sitter::Node;

use super::comprehension_fixes::{dict_comprehension_fix, generator_fix};
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, BODY_FIELD, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, NAME_FIELD, TUPLE,
    named, text,
};
use crate::rules::PythonRule;

const LOGGING_PREFIX: &str = "logging.";
const LOGGER_METHODS: [&str; 8] = [
    "debug",
    "info",
    "warn",
    "warning",
    "error",
    "critical",
    "log",
    "exception",
];
const SUBPROCESS_RUN: &str = "subprocess.run";
const CHECK_KEYWORD: &str = "check";
const SET: &str = "set";
const DICT: &str = "dict";
const GENERATOR: &str = "generator_expression";
const ARGUMENT_LIST: &str = "argument_list";
const STARRED: [&str; 2] = ["list_splat", "parenthesized_list_splat"];
const PAIR_LENGTH: usize = 2;

// LOG015 (`logging.info(...)` sobre el logger raíz), PLW1510 (`subprocess.run` sin `check=`), C401
// (`set(x for x in y)`) y C402 (`dict((k, v) for ...)`).
pub fn idiom_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    let qualified = qualified_name(model, function, source);
    if qualified
        .as_deref()
        .and_then(|name| name.strip_prefix(LOGGING_PREFIX))
        .is_some_and(|method| LOGGER_METHODS.contains(&method))
    {
        reports.push(Report::at(PythonRule::LOG015, call, ""));
    }
    if qualified.as_deref() == Some(SUBPROCESS_RUN) && !has_keyword(call, CHECK_KEYWORD, source) {
        reports.push(Report::at(PythonRule::PLW1510, function, ""));
    }
    let named_function = (function.kind() == IDENTIFIER).then(|| text(function, source));
    if let Some(generator) = single_generator(call) {
        if named_function == Some(SET) && is_builtin_named(model, function, source, SET) {
            let fix = generator_fix(call, generator, SET, source);
            reports.push(Report::at(PythonRule::C401, call, "").with_fix(fix));
        }
        if named_function == Some(DICT) && yields_pairs(generator) {
            let fix = dict_comprehension_fix(call, generator, source);
            reports.push(Report::at(PythonRule::C402, call, "").with_fix(fix));
        }
    }
    reports
}

fn has_keyword(call: Node<'_>, keyword: &str, source: &[u8]) -> bool {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .filter_map(|argument| argument.child_by_field_name(NAME_FIELD))
        .any(|name| text(name, source) == keyword)
}

// `exactly_one_argument_with_matching_function` con un generador: `f(x for x in y)` o
// `f((x for x in y))`.
fn single_generator(call: Node<'_>) -> Option<Node<'_>> {
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    if arguments.kind() == GENERATOR {
        return Some(arguments);
    }
    if arguments.kind() != ARGUMENT_LIST {
        return None;
    }
    let given: Vec<Node<'_>> = named(arguments)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    match given.as_slice() {
        [generator] if generator.kind() == GENERATOR => Some(*generator),
        _ => None,
    }
}

fn yields_pairs(generator: Node<'_>) -> bool {
    generator
        .child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == TUPLE)
        .map(named)
        .is_some_and(|elements| {
            elements.len() == PAIR_LENGTH && elements.iter().all(|element| !STARRED.contains(&element.kind()))
        })
}
