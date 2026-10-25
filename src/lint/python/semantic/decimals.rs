use tree_sitter::Node;

use super::calls::call_arguments;
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, FUNCTION_FIELD, KEYWORD_ARGUMENT, NAME_FIELD, STRING, VALUE_FIELD, is_string, named, string_prefix,
    text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::commented_unsafe;
use crate::rules::PythonRule;

const DECIMAL: &str = "decimal.Decimal";
const FLOAT: &str = "float";
const VALUE_KEYWORD: &str = "value";
const LIST_SPLAT: &str = "list_splat";
const PYTHON_WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];
const DIGIT_SEPARATOR: char = '_';
const SIGNS: [char; 2] = ['+', '-'];
const ZERO: char = '0';
const NEGATIVE_NAN: &str = "-nan";
const QUOTED_NAN: &str = "\"nan\"";
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
const FLOAT_LITERAL: &str = "float";
const UNARY: &str = "unary_operator";
const ARGUMENT_FIELD: &str = "argument";
const OPERATOR_FIELD: &str = "operator";
const SIGN_OPERATORS: [&str; 2] = ["+", "-"];
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];

// FURB157: `Decimal("10")` o `Decimal(float("nan"))`, que se escriben `Decimal(10)` y
// `Decimal("nan")`. Avisa en el argumento.
pub fn verbose_decimal_constructor(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if qualified_name(model, function, source).as_deref() != Some(DECIMAL) {
        return None;
    }
    let value = value_argument(call, source)?;
    let replacement = if value.kind() == CALL {
        non_finite_replacement(model, source, value)?
    } else {
        integer_replacement(&string_value(value, source)?)?
    };
    let fix = commented_unsafe(Fix::replace(value, &replacement, source), value, source);
    Some(Report::at(PythonRule::FURB157, value, "").with_fix(Some(fix)))
}

// El reemplazo de ruff para `Decimal("...")`: el signo, sin ceros a la izquierda, y si había
// separadores `_` los deja de a uno. `None` si no es un entero o es `"-0"`.
fn integer_replacement(literal: &str) -> Option<String> {
    let original = literal.trim_matches(PYTHON_WHITESPACE);
    let unsigned = original.trim_start_matches(DIGIT_SEPARATOR);
    let (sign, after_sign) = unsigned.strip_prefix(SIGNS).map_or(("", unsigned), |rest| {
        (unsigned.get(..1).unwrap_or_default(), rest)
    });
    let separated = original.contains(DIGIT_SEPARATOR);
    let digits = after_sign.replace(DIGIT_SEPARATOR, "");
    if digits.is_empty() || digits.len() > MAXIMUM_DIGITS {
        return None;
    }
    let significant = digits.trim_start_matches(ZERO);
    if !significant.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let rest = if separated {
        normalized_separators(after_sign)
    } else {
        significant.to_owned()
    };
    match (sign, rest.is_empty()) {
        ("-", true) => None,
        (_, true) => Some(format!("{sign}{ZERO}")),
        _ => Some(format!("{sign}{rest}")),
    }
}

// `normalize_digit_separators`: sin `_` ni ceros al principio, sin `_` al final y de a un `_`.
fn normalized_separators(digits: &str) -> String {
    let trimmed = digits
        .trim_start_matches([DIGIT_SEPARATOR, ZERO])
        .trim_end_matches(DIGIT_SEPARATOR);
    let mut normalized = String::new();
    for character in trimmed.chars() {
        if character == DIGIT_SEPARATOR && normalized.ends_with(DIGIT_SEPARATOR) {
            continue;
        }
        normalized.push(character);
    }
    normalized
}

// `Decimal(float("nan"))` pasa a `Decimal("nan")`, con el mismo string; `"-nan"` queda `"nan"`.
fn non_finite_replacement(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<String> {
    if !is_non_finite_float_call(model, source, call) {
        return None;
    }
    let argument = unparenthesized(*call_arguments(call).positional.first()?);
    let literal = string_value(argument, source)?;
    Some(if literal.trim().eq_ignore_ascii_case(NEGATIVE_NAN) {
        String::from(QUOTED_NAN)
    } else {
        text(argument, source).to_owned()
    })
}

// RUF032: `Decimal(0.1)`, que arrastra el error de representación del float. Avisa en el argumento.
pub fn decimal_from_float_literal(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let first = call_arguments(call)
        .positional
        .first()
        .copied()
        .map(unparenthesized)?;
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let is_decimal = qualified_name(model, function, source).as_deref() == Some(DECIMAL);
    (is_decimal && is_float_literal(first, source)).then(|| Report::at(PythonRule::RUF032, first, ""))
}

// Un float escrito, con signos `+`/`-` adelante; un complejo (`1.5j`) no cuenta.
fn is_float_literal(node: Node<'_>, source: &[u8]) -> bool {
    let mut current = node;
    while current.kind() == UNARY
        && current
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| SIGN_OPERATORS.contains(&text(operator, source)))
    {
        let Some(operand) = current.child_by_field_name(ARGUMENT_FIELD) else {
            return false;
        };
        current = unparenthesized(operand);
    }
    current.kind() == FLOAT_LITERAL && !text(current, source).ends_with(IMAGINARY_SUFFIXES)
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
