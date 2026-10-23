use tree_sitter::Node;

use super::calls::call_arguments;
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, FUNCTION_FIELD, KEYWORD_ARGUMENT, NAME_FIELD, STRING, VALUE_FIELD, is_string, named, string_prefix,
    text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const DECIMAL: &str = "decimal.Decimal";
const FLOAT: &str = "float";
const VALUE_KEYWORD: &str = "value";
const LIST_SPLAT: &str = "list_splat";
const PYTHON_WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];
const DIGIT_SEPARATOR: char = '_';
const SIGNS: [char; 2] = ['+', '-'];
const NEGATIVE: char = '-';
const ZERO: char = '0';
// `sys.int_info.str_digits_check_threshold`: más dígitos que esto y `int()` falla.
const MAXIMUM_DIGITS: usize = 640;
const NON_FINITE: [&str; 9] = [
    "nan",
    "+nan",
    "-nan",
    "inf",
    "+inf",
    "-inf",
    "infinity",
    "+infinity",
    "-infinity",
];
const TEMPLATE_PREFIXES: [char; 2] = ['t', 'T'];

// FURB157: `Decimal("10")` o `Decimal(float("nan"))`, que se escriben `Decimal(10)` y
// `Decimal("nan")`. Avisa en el argumento.
pub fn verbose_decimal_constructor(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if qualified_name(model, function, source).as_deref() != Some(DECIMAL) {
        return None;
    }
    let value = value_argument(call, source)?;
    let verbose = if value.kind() == CALL {
        is_non_finite_float_call(model, source, value)
    } else {
        string_value(value, source).is_some_and(|literal| is_integer_string(&literal))
    };
    verbose.then(|| Report::at(PythonRule::FURB157, value, ""))
}

// `find_argument_value("value", 0)`.
fn value_argument<'tree>(call: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let arguments = call_arguments(call);
    let by_name = arguments
        .keywords
        .iter()
        .filter(|keyword| keyword.kind() == KEYWORD_ARGUMENT)
        .find(|keyword| {
            keyword
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == VALUE_KEYWORD)
        })
        .and_then(|keyword| keyword.child_by_field_name(VALUE_FIELD));
    by_name
        .or_else(|| {
            arguments
                .positional
                .first()
                .copied()
                .filter(|first| first.kind() != LIST_SPLAT)
        })
        .map(unparenthesized)
}

fn string_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = if node.kind() == STRING {
        vec![node]
    } else {
        named(node)
    };
    let is_template = parts
        .iter()
        .any(|part| string_prefix(*part, source).contains(TEMPLATE_PREFIXES));
    if !is_string(node) || is_template {
        return None;
    }
    text_value(node, source)
}

// El texto, sin espacios, guiones bajos ni signo, son sólo dígitos; `"-0"` no cuenta porque
// `Decimal("-0")` no es `Decimal(0)`.
fn is_integer_string(literal: &str) -> bool {
    let trimmed = literal.trim_matches(PYTHON_WHITESPACE);
    let unsigned = trimmed.trim_start_matches(DIGIT_SEPARATOR);
    let negative = unsigned.starts_with(NEGATIVE);
    let digits = unsigned
        .strip_prefix(SIGNS)
        .unwrap_or(unsigned)
        .replace(DIGIT_SEPARATOR, "");
    if digits.is_empty() || digits.len() > MAXIMUM_DIGITS {
        return false;
    }
    let significant = digits.trim_start_matches(ZERO);
    significant.bytes().all(|byte| byte.is_ascii_digit()) && !(negative && significant.is_empty())
}

// `float("nan")`, `float("-inf")` y compañía, con un único argumento posicional.
fn is_non_finite_float_call(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    let is_float = call
        .child_by_field_name(FUNCTION_FIELD)
        .is_some_and(|function| is_builtin_named(model, function, source, FLOAT));
    let arguments = call_arguments(call);
    let [argument] = arguments.positional.as_slice() else {
        return false;
    };
    is_float
        && arguments.keywords.is_empty()
        && string_value(unparenthesized(*argument), source).is_some_and(|literal| {
            NON_FINITE
                .iter()
                .any(|name| literal.trim().eq_ignore_ascii_case(name))
        })
}
