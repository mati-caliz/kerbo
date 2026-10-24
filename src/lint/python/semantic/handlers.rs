use tree_sitter::Node;

use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::ast::{
    ALIAS_FIELD, ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, BLOCK, CALL, CLASS, COMMENT, DECORATED,
    EXPRESSION_STATEMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, NAME_FIELD, OBJECT_FIELD,
    TUPLE, VALUE_FIELD, named, text, unparenthesized,
};

const AS_PATTERN: &str = "as_pattern";
const TRY: &str = "try_statement";
const RAISE: &str = "raise_statement";
const CAUSE_FIELD: &str = "cause";
const BLIND_EXCEPTIONS: [&str; 2] = ["BaseException", "Exception"];
const BUILTIN_EXCEPTIONS: [&str; 2] = ["builtins.BaseException", "builtins.Exception"];
pub const PASS: &str = "pass_statement";
pub const CONTINUE: &str = "continue_statement";
const NOT_VISITED: [&str; 4] = [TRY, FUNCTION, CLASS, DECORATED];
const EXCEPTION_METHOD: &str = "exception";
const LOGGER_METHODS: [&str; 8] = [
    "debug",
    "info",
    "warn",
    "warning",
    "error",
    "critical",
    "log",
    EXCEPTION_METHOD,
];
const LOGGER_FACTORIES: [&str; 2] = ["logging.getLogger", "logging.Logger"];
const LOGGER_MODULES: [&str; 2] = ["logging", "flask.current_app.logger"];
const LOGGING_PREFIX: &str = "logging.";
const EXC_INFO: &str = "exc_info";
const FALSY: [&str; 5] = ["false", "none", "tuple", "list", "dictionary"];
const LOGGER_PREFIXES: [&str; 2] = ["log", "LOG"];
const LOGGER_SUFFIXES: [&str; 4] = ["logger", "logging", "LOGGER", "LOGGING"];

// BLE001: un `except Exception` (o `BaseException`) que no vuelve a lanzar ni registra el error.
// Devuelve dónde avisar.
pub fn blind_except<'tree>(model: &Model<'_>, source: &[u8], handler: Node<'tree>) -> Option<Node<'tree>> {
    let value = handler.child_by_field_name(VALUE_FIELD)?;
    let (exception, name) = if value.kind() == AS_PATTERN {
        let alias = value
            .child_by_field_name(ALIAS_FIELD)
            .and_then(|alias| named(alias).into_iter().find(|inner| inner.kind() == IDENTIFIER));
        (
            named(value).first().copied()?,
            alias.map(|alias| text(alias, source)),
        )
    } else {
        (value, None)
    };
    let blind = blind_exception(model, source, exception)?;
    let body: Vec<Node<'_>> = named(handler)
        .into_iter()
        .filter(|child| child.kind() == BLOCK)
        .collect();
    let statements = visited_statements(&body);
    let is_handled = statements
        .iter()
        .any(|statement| reraises(*statement, name, source) || logs_exception(model, source, *statement));
    (!is_handled).then_some(blind)
}

fn blind_exception<'tree>(model: &Model<'_>, source: &[u8], exception: Node<'tree>) -> Option<Node<'tree>> {
    let exception = unparenthesized(exception);
    if exception.kind() == TUPLE {
        return named(exception)
            .into_iter()
            .find_map(|element| blind_exception(model, source, element));
    }
    is_blind(model, source, exception).then_some(exception)
}

// Las sentencias del cuerpo y de sus bloques, sin entrar a `try`, funciones ni clases.
fn visited_statements<'tree>(blocks: &[Node<'tree>]) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let mut pending: Vec<Node<'tree>> = blocks.to_vec();
    while let Some(node) = pending.pop() {
        if NOT_VISITED.contains(&node.kind()) {
            continue;
        }
        if node.kind() != BLOCK {
            found.push(node);
        }
        if node.kind() != EXPRESSION_STATEMENT {
            pending.extend(named(node));
        }
    }
    found
}

fn reraises(statement: Node<'_>, name: Option<&str>, source: &[u8]) -> bool {
    if statement.kind() != RAISE {
        return false;
    }
    let cause = statement.child_by_field_name(CAUSE_FIELD);
    let exception = named(statement)
        .into_iter()
        .find(|child| cause.is_none_or(|cause| cause.id() != child.id()));
    let names = |node: Option<Node<'_>>| {
        node.is_some_and(|node| {
            node.kind() == IDENTIFIER && name.is_some_and(|name| text(node, source) == name)
        })
    };
    (exception.is_none() && cause.is_none()) || names(cause) || names(exception)
}

fn logs_exception(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> bool {
    if statement.kind() != EXPRESSION_STATEMENT {
        return false;
    }
    let children = named(statement);
    let [call] = children.as_slice() else {
        return false;
    };
    let Some(function) = Some(*call)
        .filter(|call| call.kind() == CALL)
        .and_then(|call| call.child_by_field_name(FUNCTION_FIELD))
    else {
        return false;
    };
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD);
    match function.kind() {
        ATTRIBUTE => {
            let method = function
                .child_by_field_name(ATTRIBUTE_FIELD)
                .map_or("", |method| text(method, source));
            is_logger_candidate(model, source, function)
                && (method == EXCEPTION_METHOD
                    || (LOGGER_METHODS.contains(&method) && has_exc_info(arguments, source)))
        }
        IDENTIFIER => qualified_name(model, function, source).is_some_and(|name| {
            name.strip_prefix(LOGGING_PREFIX).is_some_and(|method| {
                method == EXCEPTION_METHOD
                    || (LOGGER_METHODS.contains(&method) && has_exc_info(arguments, source))
            })
        }),
        _ => false,
    }
}

// `is_logger_candidate` de ruff.
fn is_logger_candidate(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    let Some(object) = function.child_by_field_name(OBJECT_FIELD) else {
        return false;
    };
    if object.kind() == CALL {
        return object
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|factory| qualified_name(model, factory, source))
            .is_some_and(|name| LOGGER_FACTORIES.contains(&name.as_str()));
    }
    if let Some(name) = qualified_name(model, object, source) {
        return LOGGER_MODULES.contains(&name.as_str());
    }
    let is_dotted = matches!(object.kind(), IDENTIFIER | ATTRIBUTE);
    let tail = text(object, source).rsplit('.').next().unwrap_or_default();
    is_dotted
        && (LOGGER_PREFIXES.iter().any(|prefix| tail.starts_with(prefix))
            || LOGGER_SUFFIXES.iter().any(|suffix| tail.ends_with(suffix)))
}

// `exc_info=` presente y no falso de forma segura (`False`, `None`, `0`, `""`, vacíos).
fn has_exc_info(arguments: Option<Node<'_>>, source: &[u8]) -> bool {
    arguments.is_some_and(|arguments| {
        named(arguments)
            .into_iter()
            .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
            .filter(|argument| {
                argument
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == EXC_INFO)
            })
            .filter_map(|argument| argument.child_by_field_name(VALUE_FIELD))
            .any(|value| !is_falsy(unparenthesized(value), source))
    })
}

fn is_falsy(value: Node<'_>, source: &[u8]) -> bool {
    let literal = text(value, source);
    match value.kind() {
        "integer" | "float" => literal.chars().all(|digit| matches!(digit, '0' | '.' | '_')),
        "string" => {
            literal
                .trim_start_matches(|character: char| character.is_ascii_alphabetic())
                .len()
                <= 2
        }
        kind => FALSY.contains(&kind) && (kind == "false" || kind == "none" || named(value).is_empty()),
    }
}

// S110: un `except` sin tipo, o con `Exception`/`BaseException`, cuyo cuerpo es sólo `pass`.
pub fn is_silenced_exception(model: &Model<'_>, source: &[u8], handler: Node<'_>, only: &str) -> bool {
    let body: Vec<Node<'_>> = named(handler)
        .into_iter()
        .filter(|child| child.kind() == BLOCK)
        .flat_map(named)
        .filter(|statement| statement.kind() != COMMENT)
        .collect();
    let only_pass = matches!(body.as_slice(), [statement] if statement.kind() == only);
    let untyped = handler.child_by_field_name(VALUE_FIELD).is_none_or(|value| {
        let exception = if value.kind() == AS_PATTERN {
            named(value).first().copied()
        } else {
            Some(value)
        };
        exception.is_some_and(|exception| {
            let candidates = if exception.kind() == TUPLE {
                named(exception)
            } else {
                vec![exception]
            };
            candidates
                .into_iter()
                .any(|candidate| is_blind(model, source, candidate))
        })
    });
    only_pass && untyped
}

// TRY002: `raise Exception(...)` o `raise BaseException`, en vez de una excepción propia.
pub fn vanilla_raise<'tree>(model: &Model<'_>, source: &[u8], statement: Node<'tree>) -> Option<Node<'tree>> {
    let cause = statement.child_by_field_name(CAUSE_FIELD);
    let exception = named(statement)
        .into_iter()
        .find(|child| child.kind() != COMMENT && cause.is_none_or(|cause| cause.id() != child.id()))?;
    let class = if exception.kind() == CALL {
        exception.child_by_field_name(FUNCTION_FIELD)?
    } else {
        exception
    };
    let is_builtin_attribute =
        qualified_name(model, class, source).is_some_and(|name| BUILTIN_EXCEPTIONS.contains(&name.as_str()));
    (is_blind(model, source, class) || is_builtin_attribute).then_some(exception)
}

fn is_blind(model: &Model<'_>, source: &[u8], exception: Node<'_>) -> bool {
    BLIND_EXCEPTIONS
        .iter()
        .any(|builtin| is_builtin_named(model, exception, source, builtin))
}
