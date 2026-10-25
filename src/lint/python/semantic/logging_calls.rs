use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_argument, keyword_value};
use super::call_fixes::builtin_available;
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use super::short_circuit::literal_truthiness;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, CLASS, FUNCTION, FUNCTION_FIELD, IDENTIFIER,
    KEYWORD_ARGUMENT, LAMBDA, NAME_FIELD, OBJECT_FIELD, TRUE, VALUE_FIELD, is_string, named, text,
    unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::{Parentheses, remove_argument};
use crate::rules::PythonRule;

const LOGGING: &str = "logging";
const LOGGING_PREFIX: &str = "logging.";
const LOGGER_FACTORIES: [&str; 2] = ["logging.getLogger", "logging.Logger"];
const FLASK_LOGGER: &str = "flask.current_app.logger";
const LOGGER_PREFIXES: [&str; 2] = ["log", "LOG"];
const LOGGER_SUFFIXES: [&str; 4] = ["logger", "logging", "LOGGER", "LOGGING"];
const LOG_METHOD: &str = "log";
const WARN: &str = "warn";
const WARNING: &str = "warning";
const MODULE_NAME: &str = "__name__";
const ERROR: &str = "error";
const EXCEPTION: &str = "exception";
const LEVELS: [&str; 7] = ["debug", "info", WARN, "warning", ERROR, EXCEPTION, "critical"];
const EXTRA: &str = "extra";
const EXC_INFO: &str = "exc_info";
const NAME: &str = "name";
const SYS_EXC_INFO: &str = "sys.exc_info";
const DICT: &str = "dict";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const GET_LOGGER: &str = "logging.getLogger";
const FILE_DUNDERS: [&str; 2] = ["__file__", "__cached__"];
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];
const RESERVED_ATTRIBUTES: [&str; 22] = [
    "args",
    "asctime",
    "created",
    "exc_info",
    "exc_text",
    "filename",
    "funcName",
    "levelname",
    "levelno",
    "lineno",
    "module",
    "msecs",
    "message",
    "msg",
    "name",
    "pathname",
    "process",
    "processName",
    "relativeCreated",
    "stack_info",
    "thread",
    "threadName",
];

struct LoggingCall<'tree> {
    method: &'tree str,
    at: Node<'tree>,
}

struct Logged<'tree> {
    call: Node<'tree>,
    function: Node<'tree>,
    given: Arguments<'tree>,
    logging: LoggingCall<'tree>,
}

// Llamadas de logging: G010 (`warn`), G101 (un `extra` que pisa un atributo de `LogRecord`), G201
// (`error(..., exc_info=True)` en un `except`), G202 (`exception(..., exc_info=True)`), LOG002
// (`getLogger(__file__)`) y LOG014 (`exc_info=True` fuera de un `except`).
pub fn logging_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let mut reports: Vec<Report> = file_logger_name(model, source, function, &given)
        .into_iter()
        .collect();
    let Some(logging) = logging_call(model, source, function) else {
        return reports;
    };
    if logging.method == WARN {
        let fix = Fix::replace(logging.at, WARNING, source);
        reports.push(Report::at(PythonRule::G010, logging.at, "").with_fix(Some(fix)));
    }
    if let Some(extra) = keyword_value(&given, EXTRA, source) {
        reports.extend(extra_attribute_clashes(model, source, extra));
    }
    let logged = Logged {
        call,
        function,
        given,
        logging,
    };
    reports.extend(exc_info_in_handler(model, source, &logged));
    reports.extend(exc_info_outside_handler(source, &logged));
    reports
}

// `find_logging_call` de ruff: `logger.info(...)` sobre algo que parece un logger, o `info(...)`
// importado de `logging`. Avisa en el método o en el nombre.
fn logging_call<'tree>(
    model: &Model<'_>,
    source: &'tree [u8],
    function: Node<'tree>,
) -> Option<LoggingCall<'tree>> {
    match function.kind() {
        ATTRIBUTE => {
            let attribute = function.child_by_field_name(ATTRIBUTE_FIELD)?;
            let method = text(attribute, source);
            ((method == LOG_METHOD || LEVELS.contains(&method))
                && is_logger_candidate(model, source, function))
            .then_some(LoggingCall {
                method,
                at: attribute,
            })
        }
        IDENTIFIER => {
            let name = qualified_name(model, function, source)?;
            let member = name.strip_prefix(LOGGING_PREFIX)?;
            let method = LEVELS
                .into_iter()
                .chain(std::iter::once(LOG_METHOD))
                .find(|level| *level == member)?;
            Some(LoggingCall { method, at: function })
        }
        _ => None,
    }
}

// `is_logger_candidate` de ruff: un logger recién creado, el módulo `logging`, el de Flask, o un nombre
// del propio archivo que parece un logger.
fn is_logger_candidate(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    let Some(object) = function.child_by_field_name(OBJECT_FIELD).map(unparenthesized) else {
        return false;
    };
    if object.kind() == CALL {
        return object
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|factory| qualified_name(model, factory, source))
            .is_some_and(|name| LOGGER_FACTORIES.contains(&name.as_str()));
    }
    if let Some(name) = qualified_name(model, object, source) {
        return name == LOGGING || name == FLASK_LOGGER;
    }
    unqualified_tail(object, source).is_some_and(|tail| {
        LOGGER_PREFIXES.iter().any(|prefix| tail.starts_with(prefix))
            || LOGGER_SUFFIXES.iter().any(|suffix| tail.ends_with(suffix))
    })
}

// El último nombre de `a.b.c`, si la expresión es sólo nombres y atributos.
fn unqualified_tail<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let mut current = node;
    while current.kind() == ATTRIBUTE {
        current = unparenthesized(current.child_by_field_name(OBJECT_FIELD)?);
    }
    if current.kind() != IDENTIFIER {
        return None;
    }
    match node.kind() {
        ATTRIBUTE => node
            .child_by_field_name(ATTRIBUTE_FIELD)
            .map(|attribute| text(attribute, source)),
        _ => Some(text(node, source)),
    }
}

// G101: las claves de `extra={...}` o `extra=dict(...)` que ya son atributos de `LogRecord`.
fn extra_attribute_clashes(model: &Model<'_>, source: &[u8], extra: Node<'_>) -> Vec<Report> {
    let extra = unparenthesized(extra);
    match extra.kind() {
        DICTIONARY => named(extra)
            .into_iter()
            .filter(|entry| entry.kind() == PAIR)
            .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
            .filter(|key| is_string(*key))
            .filter_map(|key| {
                let value = text_value(key, source)?;
                RESERVED_ATTRIBUTES
                    .contains(&value.as_str())
                    .then(|| Report::at(PythonRule::G101, key, &value))
            })
            .collect(),
        CALL if extra
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(model, function, source, DICT)) =>
        {
            arguments(extra)
                .keywords
                .into_iter()
                .filter(|keyword| keyword.kind() == KEYWORD_ARGUMENT)
                .filter_map(|keyword| {
                    let name = text(keyword.child_by_field_name(NAME_FIELD)?, source);
                    RESERVED_ATTRIBUTES
                        .contains(&name)
                        .then(|| Report::at(PythonRule::G101, keyword, name))
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

// G201 y G202, sólo adentro de un `except` de la misma función: `exc_info=True` o
// `exc_info=sys.exc_info()` en `error` (que es `exception`) o en `exception` (donde sobra, y avisa en el
// argumento).
fn exc_info_in_handler(model: &Model<'_>, source: &[u8], logged: &Logged<'_>) -> Option<Report> {
    let Logged {
        call, given, logging, ..
    } = logged;
    let in_handler = std::iter::successors(call.parent(), Node::parent)
        .take_while(|ancestor| !SCOPES.contains(&ancestor.kind()))
        .any(|ancestor| EXCEPT_CLAUSES.contains(&ancestor.kind()));
    if !in_handler {
        return None;
    }
    let keyword = keyword_argument(given, EXC_INFO, source)?;
    let value = unparenthesized(keyword.child_by_field_name(VALUE_FIELD)?);
    let enabled = value.kind() == TRUE
        || (value.kind() == CALL
            && value
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| qualified_name(model, function, source))
                .is_some_and(|name| name == SYS_EXC_INFO));
    if !enabled {
        return None;
    }
    match logging.method {
        ERROR => Some(Report::at(PythonRule::G201, logging.at, "")),
        EXCEPTION => Some(Report::at(PythonRule::G202, keyword, "")),
        _ => None,
    }
}

// LOG014: `exc_info=` con un literal verdadero fuera de todo `except`, que adjunta `None` como
// excepción. `logger.log(...)` no cuenta; `log(...)` importado de `logging`, sí.
fn exc_info_outside_handler(source: &[u8], logged: &Logged<'_>) -> Option<Report> {
    let Logged {
        call,
        function,
        given,
        logging,
    } = logged;
    if function.kind() == ATTRIBUTE && logging.method == LOG_METHOD {
        return None;
    }
    let in_handler = std::iter::successors(call.parent(), Node::parent)
        .any(|ancestor| EXCEPT_CLAUSES.contains(&ancestor.kind()));
    if in_handler {
        return None;
    }
    let keyword = keyword_argument(given, EXC_INFO, source)?;
    let value = keyword.child_by_field_name(VALUE_FIELD)?;
    if literal_truthiness(value, source) != Some(true) {
        return None;
    }
    let fix = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(|arguments| remove_argument(keyword, arguments, Parentheses::Preserve, source).marked_unsafe());
    Some(Report::at(PythonRule::LOG014, keyword, "").with_fix(fix))
}

// LOG002: `logging.getLogger(__file__)` o con `__cached__`, que dan una ruta y no el nombre del módulo.
fn file_logger_name(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    given: &Arguments<'_>,
) -> Option<Report> {
    let name = keyword_value(given, NAME, source).or_else(|| given.positional.first().copied())?;
    let name = unparenthesized(name);
    let is_file_dunder = name.kind() == IDENTIFIER
        && FILE_DUNDERS.contains(&text(name, source))
        && is_builtin_named(model, name, source, text(name, source));
    if !(is_file_dunder && qualified_name(model, function, source).is_some_and(|callee| callee == GET_LOGGER))
    {
        return None;
    }
    let fix = builtin_available(model, MODULE_NAME)
        .then(|| Fix::replace(name, MODULE_NAME, source).marked_unsafe());
    Some(Report::at(PythonRule::LOG002, name, "").with_fix(fix))
}
