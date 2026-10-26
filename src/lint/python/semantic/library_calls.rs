use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::cast_calls::is_assigned;
use super::library_fixes::{library_fix, utc_suffix_fix};
use super::model::{BindingKind, Model, Resolution};
use super::mutability::{Resolver, without_subscript};
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LEFT_FIELD,
    NAME_FIELD, OBJECT_FIELD, RIGHT_FIELD, SUBSCRIPT, SUBSCRIPT_FIELD, TRUE, fields, is_bytes, is_f_string,
    is_string, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::known_types::{Known, is_known};
use crate::rules::PythonRule;

const SUPPRESS: &str = "contextlib.suppress";
const CONTEXT_VAR: &str = "contextvars.ContextVar";
const DEFAULT: &str = "default";
const REVERSIBLES: [&str; 3] = ["reversed", "set", "sorted"];
const SLICE: &str = "slice";
const REVERSE_STEP: &str = "-1";
const SLICE_SEPARATOR: char = ':';
const ZIP: &str = "zip";
const STRICT: &str = "strict";
const KEYS: &str = "keys";
const VALUES: &str = "values";
const FROMISOFORMAT: &str = "datetime.datetime.fromisoformat";
const REPLACE: &str = "replace";
const STRIPS: [&str; 2] = ["strip", "rstrip"];
const UTC_SUFFIX: &str = "Z";
const ZERO_OFFSETS: [&str; 6] = ["+00:00", "+0000", "+00", "-00:00", "-0000", "-00"];
const LAST_DROPPED: &str = ":-1";
const PLUS: &str = "+";
const OPERATOR_FIELD: &str = "operator";
const RESOLVE: &str = "resolve";
const CURRENT_DIRECTORIES: [&str; 2] = ["", "."];
const PATH: &str = "pathlib.Path";
const PATH_CLASSES: [&str; 6] = [
    "pathlib.Path",
    "pathlib.PurePath",
    "pathlib.PosixPath",
    "pathlib.PurePosixPath",
    "pathlib.WindowsPath",
    "pathlib.PureWindowsPath",
];
const HEX: &str = "hex";
const DIGEST: &str = "digest";
const HASHES: [&str; 14] = [
    "hashlib.md5",
    "hashlib.sha1",
    "hashlib.sha224",
    "hashlib.sha256",
    "hashlib.sha384",
    "hashlib.sha512",
    "hashlib.blake2b",
    "hashlib.blake2s",
    "hashlib.sha3_224",
    "hashlib.sha3_256",
    "hashlib.sha3_384",
    "hashlib.sha3_512",
    "hashlib.shake_128",
    "hashlib.shake_256",
];
const WITH_SUFFIX: &str = "with_suffix";
const SUFFIX: &str = "suffix";
const SUFFIX_DOT: char = '.';
const VARIANCES: [&str; 2] = ["covariant", "contravariant"];
const TYPE_VARIABLES: [&str; 2] = ["TypeVar", "ParamSpec"];
const BINARY_OPERATOR: &str = "binary_operator";

// Llamadas a la biblioteca estándar que no hacen lo que parecen: B022 (`contextlib.suppress()` sin
// excepciones), B039 (`ContextVar` con un default mutable, que avisa en el default), C415
// (`sorted(x[::-1])`), SIM911 (`zip(d.keys(), d.values())`), FURB162 (`fromisoformat` con el `Z`
// reemplazado a mano, que avisa en el argumento), FURB177 (`Path().resolve()`), FURB181
// (`.digest().hex()`), PTH210 (`with_suffix` sin punto) y PLC0131 (`TypeVar` covariante y
// contravariante, que avisa en la función).
pub fn library_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let given = arguments(call);
    let resolver = Resolver {
        model,
        source,
        scope: model.scope,
    };
    let qualified = qualified_name(model, without_subscript(function), source);
    let mut reports = Vec::new();
    match qualified.as_deref() {
        Some(SUPPRESS) if given.positional.is_empty() => reports.push(Report::at(PythonRule::B022, call, "")),
        Some(CONTEXT_VAR) => reports.extend(
            keyword_value(&given, DEFAULT, source)
                .map(unparenthesized)
                .filter(|default| is_mutable_default(&resolver, *default))
                .map(|default| Report::at(PythonRule::B039, default, "")),
        ),
        Some(FROMISOFORMAT) => reports.extend(
            match (given.positional.as_slice(), given.keywords.as_slice()) {
                ([argument], []) if replaces_utc_suffix(source, unparenthesized(*argument)) => {
                    Some(*argument)
                }
                _ => None,
            }
            .map(|argument| {
                Report::at(PythonRule::FURB162, argument, "").with_fix(utc_suffix_fix(argument, source))
            }),
        ),
        Some(name)
            if TYPE_VARIABLES.iter().any(|member| is_typing(name, member))
                && is_assigned(call)
                && VARIANCES.iter().all(|variance| is_true(&given, variance, source)) =>
        {
            reports.push(Report::at(PythonRule::PLC0131, function, ""));
        }
        _ => {}
    }
    let rule = if is_reversed_argument(model, source, function, &given) {
        Some(PythonRule::C415)
    } else if is_zip_of_keys_and_values(model, source, function, &given) {
        Some(PythonRule::SIM911)
    } else if is_implicit_cwd(model, source, function, &given) {
        Some(PythonRule::FURB177)
    } else if is_digest_hex(model, source, function, &given) {
        Some(PythonRule::FURB181)
    } else if is_dotless_suffix(model, source, function, &given) {
        Some(PythonRule::PTH210)
    } else {
        None
    };
    reports
        .extend(rule.map(|rule| Report::at(rule, call, "").with_fix(library_fix(rule, model, call, source))));
    reports
}

fn is_mutable_default(resolver: &Resolver<'_, '_>, default: Node<'_>) -> bool {
    resolver.is_mutable_expression(default)
        || (default.kind() == CALL
            && default
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| !resolver.is_immutable_function(function)))
}

fn is_true(given: &Arguments<'_>, keyword: &str, source: &[u8]) -> bool {
    keyword_value(given, keyword, source).is_some_and(|value| unparenthesized(value).kind() == TRUE)
}

fn method_name<'source>(function: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    (function.kind() == ATTRIBUTE)
        .then(|| function.child_by_field_name(ATTRIBUTE_FIELD))
        .flatten()
        .map(|attribute| text(attribute, source))
}

fn plain_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    (is_string(node) && !is_f_string(node, source) && !is_bytes(node, source))
        .then(|| text_value(node, source))
        .flatten()
}

fn is_reversed_argument(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    let Some(first) = given.positional.first().map(|first| unparenthesized(*first)) else {
        return false;
    };
    let reversed = first.kind() == SUBSCRIPT
        && matches!(fields(first, SUBSCRIPT_FIELD).as_slice(), [slice] if slice.kind() == SLICE && {
            let parts: Vec<&str> = text(*slice, source).split(SLICE_SEPARATOR).map(str::trim).collect();
            matches!(parts.as_slice(), ["", "", REVERSE_STEP])
        });
    reversed
        && REVERSIBLES
            .iter()
            .any(|builtin| is_builtin_named(model, function, source, builtin))
}

fn is_zip_of_keys_and_values(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    given: &Arguments<'_>,
) -> bool {
    let only_strict = match given.keywords.as_slice() {
        [] => true,
        [keyword] => {
            keyword.kind() == KEYWORD_ARGUMENT
                && keyword
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == STRICT)
        }
        _ => false,
    };
    let [keys, values] = given.positional.as_slice() else {
        return false;
    };
    let view = |node: Node<'_>, wanted: &str| -> Option<String> {
        let node = unparenthesized(node);
        let method = node
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|_| node.kind() == CALL)?;
        let object = method.child_by_field_name(OBJECT_FIELD)?;
        let no_arguments = {
            let inner = arguments(node);
            inner.positional.is_empty() && inner.keywords.is_empty()
        };
        (method_name(method, source) == Some(wanted) && object.kind() == IDENTIFIER && no_arguments)
            .then(|| text(object, source).to_owned())
    };
    let dictionary = keys
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|method| method.child_by_field_name(OBJECT_FIELD));
    only_strict
        && view(*keys, KEYS).is_some_and(|name| view(*values, VALUES) == Some(name))
        && is_builtin_named(model, function, source, ZIP)
        && dictionary.is_some_and(|dictionary| is_known(dictionary, Known::Dict, source))
}

// `fecha.replace("Z", "+00:00")`, `fecha.rstrip("Z") + "+00:00"` o `fecha[:-1] + "+00:00"`.
fn replaces_utc_suffix(source: &[u8], argument: Node<'_>) -> bool {
    let is_zero_offset = |node: Node<'_>| {
        plain_string(unparenthesized(node), source)
            .is_some_and(|offset| ZERO_OFFSETS.contains(&offset.as_str()))
    };
    let is_utc =
        |node: Node<'_>| plain_string(unparenthesized(node), source).is_some_and(|value| value == UTC_SUFFIX);
    match argument.kind() {
        CALL => {
            let given = arguments(argument);
            argument
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|method| method_name(method, source) == Some(REPLACE))
                && given.keywords.is_empty()
                && matches!(given.positional.as_slice(), [utc, offset] if is_utc(*utc) && is_zero_offset(*offset))
        }
        BINARY_OPERATOR => {
            let operator = argument
                .child_by_field_name(OPERATOR_FIELD)
                .map(|operator| text(operator, source));
            let (Some(left), Some(right)) = (
                argument.child_by_field_name(LEFT_FIELD).map(unparenthesized),
                argument.child_by_field_name(RIGHT_FIELD),
            ) else {
                return false;
            };
            let strips = left.kind() == CALL && {
                let given = arguments(left);
                left.child_by_field_name(FUNCTION_FIELD)
                    .and_then(|method| method_name(method, source))
                    .is_some_and(|method| STRIPS.contains(&method))
                    && given.keywords.is_empty()
                    && matches!(given.positional.as_slice(), [utc] if is_utc(*utc))
            };
            let drops_last = left.kind() == SUBSCRIPT
                && matches!(fields(left, SUBSCRIPT_FIELD).as_slice(), [slice] if slice.kind() == SLICE
                    && text(*slice, source).split_whitespace().collect::<String>() == LAST_DROPPED);
            operator == Some(PLUS) && (strips || drops_last) && is_zero_offset(right)
        }
        _ => false,
    }
}

fn is_implicit_cwd(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    if method_name(function, source) != Some(RESOLVE)
        || !given.positional.is_empty()
        || !given.keywords.is_empty()
    {
        return false;
    }
    let Some(path) = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == CALL)
    else {
        return false;
    };
    let path_arguments = arguments(path);
    let current = path_arguments.keywords.is_empty()
        && match path_arguments.positional.as_slice() {
            [] => true,
            [only] => {
                plain_string(*only, source).is_some_and(|value| CURRENT_DIRECTORIES.contains(&value.as_str()))
            }
            _ => false,
        };
    current
        && path
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|factory| qualified_name(model, factory, source))
            .is_some_and(|name| name == PATH)
}

fn is_digest_hex(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    if method_name(function, source) != Some(HEX)
        || !given.positional.is_empty()
        || !given.keywords.is_empty()
    {
        return false;
    }
    let digest = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == CALL)
        .and_then(|object| object.child_by_field_name(FUNCTION_FIELD))
        .filter(|method| method_name(*method, source) == Some(DIGEST));
    digest
        .and_then(|method| method.child_by_field_name(OBJECT_FIELD))
        .filter(|hash| hash.kind() == CALL)
        .and_then(|hash| hash.child_by_field_name(FUNCTION_FIELD))
        .and_then(|constructor| qualified_name(model, constructor, source))
        .is_some_and(|name| HASHES.contains(&name.as_str()))
}

fn is_dotless_suffix(model: &Model<'_>, source: &[u8], function: Node<'_>, given: &Arguments<'_>) -> bool {
    if method_name(function, source) != Some(WITH_SUFFIX)
        || given.positional.len().saturating_add(given.keywords.len()) > 1
    {
        return false;
    }
    let Some(suffix) = keyword_value(given, SUFFIX, source)
        .or_else(|| given.positional.first().copied())
        .and_then(|suffix| plain_string(unparenthesized(suffix), source))
    else {
        return false;
    };
    let valid = suffix.is_empty() || (suffix.starts_with(SUFFIX_DOT) && suffix.len() > 1);
    !valid
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| is_path(model, source, unparenthesized(object)))
}

// `pathlib.Path(...)` y compañía, o un nombre ligado una sola vez a una de esas llamadas.
fn is_path(model: &Model<'_>, source: &[u8], node: Node<'_>) -> bool {
    let is_path_call = |call: Node<'_>| {
        call.kind() == CALL
            && call
                .child_by_field_name(FUNCTION_FIELD)
                .and_then(|factory| qualified_name(model, factory, source))
                .is_some_and(|name| PATH_CLASSES.contains(&name.as_str()))
    };
    if node.kind() != IDENTIFIER {
        return is_path_call(node);
    }
    let Resolution::Binding(id) = model.lookup(text(node, source)) else {
        return false;
    };
    model
        .binding(id)
        .filter(|binding| binding.kind == BindingKind::Assignment)
        .and_then(|binding| binding.site.node)
        .and_then(|name| name.parent())
        .filter(|assignment| assignment.kind() == ASSIGNMENT)
        .and_then(|assignment| assignment.child_by_field_name(RIGHT_FIELD))
        .is_some_and(|value| is_path_call(unparenthesized(value)))
}
