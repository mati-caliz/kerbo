use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::call_rewrites::{call_rewrite_fix, print_message_fix};
use super::model::Model;
use super::mutability::Resolver;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, OBJECT_FIELD,
    is_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const FROMKEYS: &str = "fromkeys";
const DICT: &str = "dict";
const DEFAULTDICT: &str = "collections.defaultdict";
const DEFAULT_FACTORY: &str = "default_factory";
const MAP: &str = "map";
const INT: &str = "int";
const SPLIT: &str = "split";
const SEPARATOR: &str = "sep";
const DOT: &str = ".";
const VERSION: &str = "__version__";
const STARMAP: &str = "itertools.starmap";
const ZIP: &str = "zip";
const LIST_SPLAT: &str = "list_splat";
const PRINT: &str = "print";
const NON_CALLABLE: [&str; 20] = [
    "integer",
    "float",
    "true",
    "false",
    "none",
    "ellipsis",
    "string",
    "concatenated_string",
    "list",
    "dictionary",
    "set",
    "tuple",
    "slice",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
    "unary_operator",
    "binary_operator",
    "expression_list",
];

// RUF024 (`dict.fromkeys` con un valor mutable compartido), RUF026 (`defaultdict(default_factory=...)`,
// que es posicional), RUF048 (`map(int, __version__.split("."))`) y RUF058 (`starmap` sobre un `zip`).
// Avisan en la llamada.
pub fn ruff_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let resolver = Resolver {
        model,
        source,
        scope: model.scope,
    };
    let rule = if is_mutable_fromkeys(&resolver, function, &given) {
        Some(PythonRule::RUF024)
    } else if is_keyword_default_factory(model, source, function, &given) {
        Some(PythonRule::RUF026)
    } else if is_version_parsing(model, source, function, &given) {
        Some(PythonRule::RUF048)
    } else if is_starmap_zip(model, source, function, &given) {
        Some(PythonRule::RUF058)
    } else {
        None
    };
    rule.map(|rule| Report::at(rule, call, "").with_fix(call_rewrite_fix(rule, call, &given, source)))
        .into_iter()
        .collect()
}

fn is_mutable_fromkeys(resolver: &Resolver<'_, '_>, function: Node<'_>, given: &Arguments<'_>) -> bool {
    let is_fromkeys = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, resolver.source) == FROMKEYS)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| is_builtin_named(resolver.model, object, resolver.source, DICT));
    is_fromkeys
        && given.keywords.is_empty()
        && matches!(given.positional.as_slice(), [_, value] if resolver.is_mutable_expression(*value))
}

fn is_keyword_default_factory(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    given: &Arguments<'_>,
) -> bool {
    qualified_name(model, function, source).as_deref() == Some(DEFAULTDICT)
        && given.positional.is_empty()
        && keyword_value(given, DEFAULT_FACTORY, source)
            .is_some_and(|value| !NON_CALLABLE.contains(&unparenthesized(value).kind()))
}

fn is_version_parsing(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    let [converter, split] = given.positional.as_slice() else {
        return false;
    };
    given.keywords.is_empty()
        && is_builtin_named(model, function, source, MAP)
        && is_builtin_named(model, *converter, source, INT)
        && is_version_split(source, unparenthesized(*split))
}

// `__version__.split(".")` o `x.__version__.split(sep=".")`.
fn is_version_split(source: &[u8], call: Node<'_>) -> bool {
    if call.kind() != CALL {
        return false;
    }
    let given = arguments(call);
    let separator = match (given.positional.as_slice(), given.keywords.as_slice()) {
        ([only], []) => Some(*only),
        ([], [keyword]) if keyword.kind() == KEYWORD_ARGUMENT => keyword_value(&given, SEPARATOR, source),
        _ => None,
    };
    let splits_on_dot = separator.is_some_and(|separator| {
        is_string(separator) && text_value(separator, source).is_some_and(|value| value == DOT)
    });
    let Some(method) = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|method| method.kind() == ATTRIBUTE)
    else {
        return false;
    };
    let is_split = method
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == SPLIT);
    let is_version = method.child_by_field_name(OBJECT_FIELD).is_some_and(|object| {
        let name = match object.kind() {
            IDENTIFIER => Some(object),
            ATTRIBUTE => object.child_by_field_name(ATTRIBUTE_FIELD),
            _ => None,
        };
        name.is_some_and(|name| text(name, source) == VERSION)
    });
    splits_on_dot && is_split && is_version
}

fn is_starmap_zip(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    let [_, iterable] = given.positional.as_slice() else {
        return false;
    };
    let iterable = unparenthesized(*iterable);
    if !given.keywords.is_empty() || iterable.kind() != CALL {
        return false;
    }
    let zipped = arguments(iterable);
    let only_starred = !zipped.positional.is_empty()
        && zipped
            .positional
            .iter()
            .all(|argument| argument.kind() == LIST_SPLAT);
    zipped.keywords.is_empty()
        && !only_starred
        && qualified_name(model, function, source).as_deref() == Some(STARMAP)
        && iterable
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|zip| is_builtin_named(model, zip, source, ZIP))
}

// RUF030: `assert x, print(...)`, que imprime en vez de dar el mensaje. Avisa en la llamada.
pub fn assert_with_print(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Option<Report> {
    let parts: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|part| part.kind() != COMMENT)
        .collect();
    let [_, message] = parts.as_slice() else {
        return None;
    };
    let message = unparenthesized(*message);
    (message.kind() == CALL
        && message
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(model, function, source, PRINT)))
    .then(|| Report::at(PythonRule::RUF030, message, "").with_fix(print_message_fix(message, source)))
}
