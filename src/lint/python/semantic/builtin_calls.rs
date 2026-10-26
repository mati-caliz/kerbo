use tree_sitter::Node;

use super::call_fixes::type_of_primitive_fix;
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use super::reference_fixes::sys_exit_fix;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FALSE, FUNCTION_FIELD, KEYWORD_ARGUMENT,
    NAME_FIELD, PARENTHESIZED, STRING, TRUE, VALUE_FIELD, is_string, named, string_prefix, text,
    unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const FUNCTOOLS: &str = "functools";
const CACHE: &str = "cache";
const TYPE_BUILTIN: &str = "type";
const NUMBERS: [&str; 2] = ["integer", "float"];
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];
const FORMATTED_PREFIXES: [char; 2] = ['f', 'F'];
const SUBPROCESS_RUN: &str = "subprocess.run";
const SUBPROCESS_PIPE: &str = "subprocess.PIPE";
const STDOUT: &str = "stdout";
const STDERR: &str = "stderr";
const CALL_DUNDER: &str = "__call__";
const HASATTR: &str = "hasattr";
const GETATTR: &str = "getattr";
const LRU_CACHE: &str = "functools.lru_cache";
const ARGUMENT_LIST: &str = "argument_list";
const DATETIME_TODAY: &str = "datetime.datetime.today";
const DATETIME_UTCNOW: &str = "datetime.datetime.utcnow";
const EXIT_ALIASES: [&str; 2] = ["exit", "quit"];
const UNIVERSAL_NEWLINES: &str = "universal_newlines";
const SUBPROCESS_POPEN: &str = "subprocess.Popen";
const PREEXEC_FN: &str = "preexec_fn";
const MAXSIZE: &str = "maxsize";
const NONE_LITERAL: &str = "none";
const REPLACE: &str = "replace";
const ASTIMEZONE: &str = "astimezone";
const DICTIONARY_SPLAT: &str = "dictionary_splat";

pub struct Arguments<'tree> {
    pub positional: Vec<Node<'tree>>,
    pub keywords: Vec<Node<'tree>>,
}

// UP003 (`type("")`), UP022 (`subprocess.run` con `stdout` y `stderr` en `PIPE`) y B004
// (`hasattr(x, "__call__")`).
pub fn builtin_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let arguments = arguments(call);
    let mut reports = Vec::new();
    if let [argument] = arguments.positional.as_slice()
        && is_primitive_literal(*argument, source)
        && is_builtin_named(model, function, source, TYPE_BUILTIN)
    {
        let fix = type_of_primitive_fix(model, call, *argument, source);
        reports.push(Report::at(PythonRule::UP003, call, "").with_fix(fix));
    }
    let is_pipe = |keyword: &str| {
        keyword_value(&arguments, keyword, source)
            .and_then(|value| qualified_name(model, value, source))
            .is_some_and(|name| name == SUBPROCESS_PIPE)
    };
    if qualified_name(model, function, source).as_deref() == Some(SUBPROCESS_RUN)
        && is_pipe(STDOUT)
        && is_pipe(STDERR)
    {
        reports.push(Report::at(PythonRule::UP022, call, ""));
    }
    let qualified = qualified_name(model, function, source);
    let naive_datetime = match qualified.as_deref() {
        Some(DATETIME_TODAY) => Some(PythonRule::DTZ002),
        Some(DATETIME_UTCNOW) => Some(PythonRule::DTZ003),
        _ => None,
    };
    if let Some(rule) = naive_datetime
        && !is_followed_by_astimezone(call, source)
    {
        reports.push(Report::at(rule, call, ""));
    }
    if EXIT_ALIASES
        .iter()
        .any(|alias| is_builtin_named(model, function, source, alias))
    {
        let fix = sys_exit_fix(model, function, &arguments, source);
        reports.push(Report::at(PythonRule::PLR1722, function, "").with_fix(fix));
    }
    if qualified.as_deref() == Some(SUBPROCESS_RUN)
        && let Some(keyword) = keyword_argument(&arguments, UNIVERSAL_NEWLINES, source)
        && let Some(name) = keyword.child_by_field_name(NAME_FIELD)
    {
        reports.push(Report::at(PythonRule::UP021, name, ""));
    }
    if qualified.as_deref() == Some(SUBPROCESS_POPEN)
        && let Some(keyword) = keyword_argument(&arguments, PREEXEC_FN, source)
        && keyword
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| unparenthesized(value).kind() != NONE_LITERAL)
    {
        reports.push(Report::at(PythonRule::PLW1509, keyword, ""));
    }
    if is_unreliable_callable_check(model, source, function, &arguments) {
        reports.push(Report::at(PythonRule::B004, call, ""));
    }
    reports
}

// `followed_by_astimezone`: `datetime.today()` seguido de `.astimezone()`, quizás después de una
// cadena de `.replace(...)`.
fn is_followed_by_astimezone(call: Node<'_>, source: &[u8]) -> bool {
    let mut current = call;
    loop {
        let Some(attribute) = parent_expression(current).filter(|parent| parent.kind() == ATTRIBUTE) else {
            return false;
        };
        match attribute
            .child_by_field_name(ATTRIBUTE_FIELD)
            .map(|method| text(method, source))
        {
            Some(ASTIMEZONE) => return true,
            Some(REPLACE) => {}
            _ => return false,
        }
        let Some(next) = parent_expression(attribute).filter(|parent| {
            parent.kind() == CALL
                && parent
                    .child_by_field_name(FUNCTION_FIELD)
                    .is_some_and(|function| function.id() == attribute.id())
        }) else {
            return false;
        };
        current = next;
    }
}

fn parent_expression(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent).find(|ancestor| ancestor.kind() != PARENTHESIZED)
}

// UP011: `@functools.lru_cache()` sin argumentos, que se escribe sin paréntesis. Avisa en los
// paréntesis.
pub fn lru_cache_reports(model: &Model<'_>, source: &[u8], decorators: &[Node<'_>]) -> Vec<Report> {
    decorators
        .iter()
        .filter_map(|decorator| {
            named(*decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
                .filter(|expression| expression.kind() == CALL)
        })
        .filter_map(|call| {
            let function = call.child_by_field_name(FUNCTION_FIELD)?;
            let list = call.child_by_field_name(ARGUMENTS_FIELD)?;
            if list.kind() != ARGUMENT_LIST
                || qualified_name(model, function, source).as_deref() != Some(LRU_CACHE)
            {
                return None;
            }
            let given: Vec<Node<'_>> = named(list)
                .into_iter()
                .filter(|argument| argument.kind() != COMMENT)
                .collect();
            match given.as_slice() {
                [] => Some(
                    Report::at(PythonRule::UP011, list, "").with_fix(Some(Fix::replace(list, "", source))),
                ),
                [keyword] if is_unbounded_maxsize(*keyword, source) => {
                    let request = ImportRequest {
                        module: FUNCTOOLS,
                        member: CACHE,
                        style: ImportStyle::Import,
                    };
                    let fix = call.parent().and_then(|decorator| {
                        let (name, import) = get_or_import_member(model, source, decorator, request)?;
                        let reference = TextEdit {
                            start: TextPoint::from_point(call.start_position()),
                            end: TextPoint::from_point(call.end_position()),
                            replacement: name,
                        };
                        Some(Fix::from_edits(source, vec![reference, import]))
                    });
                    Some(Report::at(PythonRule::UP033, list, "").with_fix(fix))
                }
                _ => None,
            }
        })
        .collect()
}

// `maxsize=None`.
fn is_unbounded_maxsize(keyword: Node<'_>, source: &[u8]) -> bool {
    keyword.kind() == KEYWORD_ARGUMENT
        && keyword
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == MAXSIZE)
        && keyword
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| unparenthesized(value).kind() == NONE_LITERAL)
}

// `Primitive::from_expr`: un booleano, un string, bytes o un número escrito.
fn is_primitive_literal(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        TRUE | FALSE => true,
        kind if NUMBERS.contains(&kind) => true,
        _ if is_string(node) => {
            let parts = if node.kind() == STRING {
                vec![node]
            } else {
                named(node)
            };
            let is_formatted = parts.iter().any(|part| {
                let prefix = string_prefix(*part, source);
                prefix.contains(TEMPLATE_PREFIXES) || prefix.contains(FORMATTED_PREFIXES)
            });
            !is_formatted
        }
        _ => false,
    }
}

// `unreliable_callable_check`: `hasattr(x, "__call__")` o `getattr(x, "__call__"[, default])`.
fn is_unreliable_callable_check(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    arguments: &Arguments<'_>,
) -> bool {
    let positional = &arguments.positional;
    let [_, attribute, ..] = positional.as_slice() else {
        return false;
    };
    let names_call = is_string(*attribute) && text_value(*attribute, source).as_deref() == Some(CALL_DUNDER);
    let valid = if is_builtin_named(model, function, source, HASATTR) {
        positional.len() == 2
    } else if is_builtin_named(model, function, source, GETATTR) {
        matches!(positional.len(), 2 | 3)
    } else {
        false
    };
    arguments.keywords.is_empty() && names_call && valid
}

pub fn arguments(call: Node<'_>) -> Arguments<'_> {
    let items = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(|list| {
            if list.kind() == ARGUMENT_LIST {
                named(list)
            } else {
                vec![list]
            }
        })
        .unwrap_or_default();
    let (keywords, positional) = items
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT));
    Arguments { positional, keywords }
}

pub fn keyword_value<'tree>(
    arguments: &Arguments<'tree>,
    keyword: &str,
    source: &[u8],
) -> Option<Node<'tree>> {
    keyword_argument(arguments, keyword, source)
        .and_then(|argument| argument.child_by_field_name(VALUE_FIELD))
}

pub fn keyword_argument<'tree>(
    arguments: &Arguments<'tree>,
    keyword: &str,
    source: &[u8],
) -> Option<Node<'tree>> {
    arguments
        .keywords
        .iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .find(|argument| {
            argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == keyword)
        })
        .copied()
}
