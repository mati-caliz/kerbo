use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::model::Model;
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::type_hints::Hints;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, CLASS, COMMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER,
    LAMBDA, LEFT_FIELD, OBJECT_FIELD, RIGHT_FIELD, TUPLE, is_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const TYPE_VARIABLES: [&str; 4] = ["TypeVar", "ParamSpec", "TypeVarTuple", "NewType"];
const NAME_KEYWORD: &str = "name";
const LOGGER_CLASS: &str = "logging.Logger";
const MAP: &str = "map";
const WRAPPERS: [&str; 3] = ["list", "set", "dict"];
const DICT: &str = "dict";
const LIST_LITERAL: &str = "list";
const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const BIN: &str = "bin";
const COUNT: &str = "count";
const ONE: &str = "1";
const ISINSTANCE: &str = "isinstance";
const TYPE: &str = "type";
const NONE_LITERAL: &str = "none";
const MATH_LOG: &str = "math.log";
const MATH_E: &str = "math.e";
const REDUNDANT_BASES: [&str; 4] = ["2", "10", "2.0", "10.0"];
const UTCFROMTIMESTAMP: &str = "datetime.datetime.utcfromtimestamp";
const EXCEPTION: &str = "exception";
const LOGGING: &str = "logging";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const AS_PATTERN: &str = "as_pattern";
const ALIAS_FIELD: &str = "alias";
const VALUE_FIELD: &str = "value";
const BLOCKING_SLEEP: &str = "time.sleep";
const BLOCKING_OPEN: [&str; 3] = ["builtins.open", "io.open", "io.open_code"];
const ZERO_SLEEPS: [&str; 2] = ["trio.sleep", "anyio.sleep"];
const ZERO: &str = "0";
const ASYNC: &str = "async";
const INTERPOLATION: &str = "interpolation";
const CONCATENATED_STRING: &str = "concatenated_string";
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];

// PLC0132 (nombre de `TypeVar` distinto de la variable), LOG001 (`logging.Logger(...)`), C417
// (`map(lambda ...)`), FURB161 (`bin(x).count("1")`), FURB168 (`isinstance(x, type(None))`), FURB163
// (`math.log(x, 2)`), DTZ004 (`utcfromtimestamp`), TRY401 (la excepción repetida en
// `logger.exception`) y ASYNC251, ASYNC230 y ASYNC115 (llamadas que bloquean en una función async).
pub fn tail_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let hints = Hints { model, source };
    let given = arguments(call);
    let qualified = qualified_name(model, function, source);
    let qualified = qualified.as_deref();
    let mut reports = Vec::new();
    if qualified.is_some_and(|name| TYPE_VARIABLES.iter().any(|member| is_typing(name, member)))
        && is_misnamed_type_variable(call, &given, source)
    {
        reports.push(Report::at(PythonRule::PLC0132, call, ""));
    }
    if qualified == Some(LOGGER_CLASS) {
        reports.push(Report::at(PythonRule::LOG001, function, ""));
    }
    reports.extend(unnecessary_map(hints, call, function, &given));
    if is_bit_count(hints, function, &given) {
        reports.push(Report::at(PythonRule::FURB161, call, ""));
    }
    if is_builtin_named(model, function, source, ISINSTANCE) && is_type_none_check(hints, &given) {
        reports.push(Report::at(PythonRule::FURB168, call, ""));
    }
    if qualified == Some(MATH_LOG) && is_redundant_log_base(hints, &given) {
        reports.push(Report::at(PythonRule::FURB163, call, ""));
    }
    if qualified == Some(UTCFROMTIMESTAMP) {
        reports.push(Report::at(PythonRule::DTZ004, call, ""));
    }
    reports.extend(verbose_log_message(hints, call, function, &given));
    reports.extend(blocking_async_call(source, call, qualified, &given));
    reports
}

// PLC0132: `T = TypeVar("U")`.
fn is_misnamed_type_variable(call: Node<'_>, given: &Arguments<'_>, source: &[u8]) -> bool {
    let Some(target) = call
        .parent()
        .filter(|parent| {
            parent.kind() == ASSIGNMENT
                && parent
                    .child_by_field_name(RIGHT_FIELD)
                    .is_some_and(|value| value.id() == call.id())
        })
        .and_then(|assignment| assignment.child_by_field_name(LEFT_FIELD))
        .filter(|target| target.kind() == IDENTIFIER)
    else {
        return false;
    };
    given
        .positional
        .first()
        .copied()
        .or_else(|| keyword_value(given, NAME_KEYWORD, source))
        .filter(|name| is_string(*name))
        .and_then(|name| text_value(name, source))
        .is_some_and(|name| name != text(target, source))
}

// C417: `map(lambda x: ..., items)`, que es una comprensión. Envuelto en `list`, `set` o `dict` avisa
// en la llamada de afuera.
fn unnecessary_map(
    hints: Hints<'_, '_, '_>,
    call: Node<'_>,
    function: Node<'_>,
    given: &Arguments<'_>,
) -> Option<Report> {
    let is_builtin = |node: Node<'_>, name: &str| is_builtin_named(hints.model, node, hints.source, name);
    if is_builtin(function, MAP) {
        let wrapped = call
            .parent()
            .and_then(|list| list.parent())
            .filter(|outer| outer.kind() == CALL)
            .and_then(|outer| outer.child_by_field_name(FUNCTION_FIELD))
            .is_some_and(|outer| WRAPPERS.iter().any(|name| is_builtin(outer, name)));
        return (!wrapped && map_lambda_body(call).is_some()).then(|| Report::at(PythonRule::C417, call, ""));
    }
    let wrapper = WRAPPERS.iter().find(|name| is_builtin(function, name))?;
    let [inner] = given.positional.as_slice() else {
        return None;
    };
    let inner = unparenthesized(*inner);
    let maps = inner.kind() == CALL
        && inner
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|inner_function| is_builtin(inner_function, MAP));
    let body = map_lambda_body(inner).filter(|_| maps && given.keywords.is_empty())?;
    let fits = *wrapper != DICT
        || (matches!(body.kind(), TUPLE | LIST_LITERAL)
            && named(body)
                .into_iter()
                .filter(|element| element.kind() != COMMENT)
                .count()
                == 2);
    fits.then(|| Report::at(PythonRule::C417, call, ""))
}

fn map_lambda_body(call: Node<'_>) -> Option<Node<'_>> {
    let given = arguments(call);
    let [function, _] = given.positional.as_slice() else {
        return None;
    };
    let lambda = unparenthesized(*function);
    if lambda.kind() != LAMBDA || !given.keywords.is_empty() {
        return None;
    }
    let parameters: Vec<Node<'_>> = lambda
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| parameter.kind() != COMMENT)
        .collect();
    let simple = matches!(parameters.as_slice(), [only] if only.kind() == IDENTIFIER);
    lambda
        .child_by_field_name(BODY_FIELD)
        .map(unparenthesized)
        .filter(|_| simple)
}

// FURB161: `bin(x).count("1")`, que es `x.bit_count()`.
fn is_bit_count(hints: Hints<'_, '_, '_>, function: Node<'_>, given: &Arguments<'_>) -> bool {
    let counts_ones = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, hints.source) == COUNT)
        && matches!(given.positional.as_slice(), [only] if is_string(*only)
            && text_value(*only, hints.source).as_deref() == Some(ONE));
    counts_ones
        && function
            .child_by_field_name(OBJECT_FIELD)
            .map(unparenthesized)
            .is_some_and(|object| {
                object.kind() == CALL
                    && object
                        .child_by_field_name(FUNCTION_FIELD)
                        .is_some_and(|inner| is_builtin_named(hints.model, inner, hints.source, BIN))
                    && arguments(object).positional.len() == 1
            })
}

// FURB168: `isinstance(x, type(None))`, que es `x is None`.
fn is_type_none_check(hints: Hints<'_, '_, '_>, given: &Arguments<'_>) -> bool {
    let [_, checked] = given.positional.as_slice() else {
        return false;
    };
    let checked = unparenthesized(*checked);
    checked.kind() == CALL
        && checked
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(hints.model, function, hints.source, TYPE))
        && matches!(arguments(checked).positional.as_slice(), [only] if unparenthesized(*only).kind() == NONE_LITERAL)
}

// FURB163: `math.log(x, 2)`, `math.log(x, 10)` o `math.log(x, math.e)`, que tienen su propia función.
fn is_redundant_log_base(hints: Hints<'_, '_, '_>, given: &Arguments<'_>) -> bool {
    let [_, base] = given.positional.as_slice() else {
        return false;
    };
    let base = unparenthesized(*base);
    REDUNDANT_BASES.contains(&text(base, hints.source))
        || qualified_name(hints.model, base, hints.source).as_deref() == Some(MATH_E)
}

// TRY401: la excepción del `except ... as e` pasada otra vez a `logger.exception`, que ya la registra.
fn verbose_log_message(
    hints: Hints<'_, '_, '_>,
    call: Node<'_>,
    function: Node<'_>,
    given: &Arguments<'_>,
) -> Vec<Report> {
    let source = hints.source;
    let is_exception_log = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == EXCEPTION)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| is_logger(hints, object));
    let Some(alias) = is_exception_log.then(|| handler_alias(call, source)).flatten() else {
        return Vec::new();
    };
    let values = given.positional.iter().copied().chain(
        given
            .keywords
            .iter()
            .filter_map(|keyword| keyword.child_by_field_name(VALUE_FIELD)),
    );
    let mut candidates = Vec::new();
    for value in values {
        candidates.push(value);
        candidates.extend(
            named(value)
                .into_iter()
                .filter(|part| part.kind() == INTERPOLATION)
                .filter_map(|interpolation| {
                    named(interpolation)
                        .into_iter()
                        .find(|inner| inner.kind() != COMMENT)
                }),
        );
        if value.kind() == CONCATENATED_STRING {
            candidates.extend(
                named(value)
                    .into_iter()
                    .flat_map(named)
                    .filter(|part| part.kind() == INTERPOLATION)
                    .filter_map(|interpolation| {
                        named(interpolation)
                            .into_iter()
                            .find(|inner| inner.kind() != COMMENT)
                    }),
            );
        }
    }
    candidates
        .into_iter()
        .filter(|node| node.kind() == IDENTIFIER && text(*node, source) == alias)
        .map(|node| Report::at(PythonRule::TRY401, node, ""))
        .collect()
}

fn handler_alias<'source>(call: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let handler = std::iter::successors(call.parent(), Node::parent)
        .take_while(|ancestor| !SCOPES.contains(&ancestor.kind()))
        .find(|ancestor| EXCEPT_CLAUSES.contains(&ancestor.kind()))?;
    let pattern = handler
        .child_by_field_name(VALUE_FIELD)
        .filter(|value| value.kind() == AS_PATTERN)?;
    let alias = pattern.child_by_field_name(ALIAS_FIELD)?;
    let name = named(alias)
        .into_iter()
        .find(|inner| inner.kind() == IDENTIFIER)
        .unwrap_or(alias);
    Some(text(name, source))
}

fn is_logger(hints: Hints<'_, '_, '_>, object: Node<'_>) -> bool {
    if let Some(name) = qualified_name(hints.model, object, hints.source) {
        return name == LOGGING;
    }
    let tail = if object.kind() == ATTRIBUTE {
        object
            .child_by_field_name(ATTRIBUTE_FIELD)
            .map(|attribute| text(attribute, hints.source))
    } else {
        (object.kind() == IDENTIFIER).then(|| text(object, hints.source))
    };
    tail.map(str::to_ascii_lowercase)
        .is_some_and(|tail| tail.starts_with("log") || tail.ends_with("logger"))
}

// ASYNC251 (`time.sleep`), ASYNC230 (`open`) y ASYNC115 (`trio.sleep(0)`) adentro de una función
// async.
fn blocking_async_call(
    source: &[u8],
    call: Node<'_>,
    qualified: Option<&str>,
    given: &Arguments<'_>,
) -> Option<Report> {
    let qualified = qualified?;
    let in_async = std::iter::successors(call.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()))
        .is_some_and(|scope| {
            scope.kind() == FUNCTION && scope.child(0).is_some_and(|first| text(first, source) == ASYNC)
        });
    if !in_async {
        return None;
    }
    let rule = if qualified == BLOCKING_SLEEP {
        PythonRule::ASYNC251
    } else if BLOCKING_OPEN.contains(&qualified) {
        PythonRule::ASYNC230
    } else if ZERO_SLEEPS.contains(&qualified)
        && matches!(given.positional.as_slice(), [only] if text(*only, source) == ZERO)
    {
        PythonRule::ASYNC115
    } else {
        return None;
    };
    Some(Report::at(rule, call, ""))
}
