use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::call_fixes::builtin_available;
use super::model::Model;
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::type_hints::Hints;
use super::unwrap_fixes::unnecessary_round_fix;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, CLASS, COMMENT, FALSE, FUNCTION, FUNCTION_FIELD, IDENTIFIER,
    KEYWORD_ARGUMENT, LAMBDA, LEFT_FIELD, NAME_FIELD, OBJECT_FIELD, RIGHT_FIELD, SUBSCRIPT, SUBSCRIPT_FIELD,
    TRUE, TUPLE, VALUE_FIELD, is_string, named, text, unparenthesized,
};
use crate::lint::python::codegen;
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::has_comment_within;
use crate::lint::python::methods::method_kind;
use crate::rules::PythonRule;

const INTEGER: &str = "integer";
const NONE_LITERAL: &str = "none";
const UNARY_OPERATOR: &str = "unary_operator";
const SLICE: &str = "slice";
const PARENTHESIZED: &str = "parenthesized_expression";
const LIST: &str = "list";
const ARGUMENT_FIELD: &str = "argument";
const OPERATOR_FIELD: &str = "operator";
const SORTED: &str = "sorted";
const ROUND: &str = "round";
const INT: &str = "int";
const PRINT: &str = "print";
const SUPER: &str = "super";
const SEP: &str = "sep";
const NDIGITS: &str = "ndigits";
const KEY: &str = "key";
const REVERSE: &str = "reverse";
const ALLOWED_SORT_KEYWORDS: [&str; 2] = [KEY, REVERSE];
const FIRST_INDEX: &str = "0";
const AUTOMATIC_BASE: &str = "0";
const LAST_INDEX: &str = "-1";
const MIN: &str = "min";
const MAX: &str = "max";
const PREFIXED_BASES: [&str; 3] = ["2", "8", "16"];
const STRIPS: [&str; 3] = ["strip", "lstrip", "rstrip"];
const NAMED_TUPLE: &str = "NamedTuple";
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];
const MINUS: &str = "-";

// Llamadas sueltas: FURB192 (`sorted(x)[0]`), RUF057 (`round` que no cambia nada), FURB166
// (`int(s[2:], 16)`), B005 (`strip` con caracteres repetidos), FURB105 (`print("")`), UP014
// (`NamedTuple` funcional) y PLW0245 (`super.x()`).
pub fn misc_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let hints = Hints { model, source };
    let given = arguments(call);
    let builtin = |name: &str| is_builtin_named(model, function, source, name);
    let mut reports = Vec::new();
    if builtin(SORTED) {
        reports.extend(sorted_min_max(model, source, call, &given));
    }
    if builtin(ROUND) && is_unnecessary_round(&given, source) {
        let fix = given
            .positional
            .first()
            .map(|number| unnecessary_round_fix(call, *number, source));
        reports.push(Report::at(PythonRule::RUF057, call, "").with_fix(fix));
    }
    if builtin(INT) && is_prefix_stripped_int(&given, source) {
        reports.push(Report::at(PythonRule::FURB166, call, "").with_fix(prefixed_base_fix(&given, source)));
    }
    if builtin(PRINT) && is_unnecessary_print_argument(&given, source) {
        reports.push(
            Report::at(PythonRule::FURB105, call, "")
                .with_fix(Some(print_fix(call, function, &given, source))),
        );
    }
    if is_repeated_strip(function, &given, source) {
        reports.push(Report::at(PythonRule::B005, call, ""));
    }
    if is_functional_named_tuple(hints, call, function, &given) {
        reports.extend(
            call.parent()
                .map(|assignment| Report::at(PythonRule::UP014, assignment, "")),
        );
    }
    reports.extend(super_without_call(model, source, call, function));
    reports
}

fn is_integer(node: Node<'_>, source: &[u8], allow_negative: bool) -> bool {
    let node = unparenthesized(node);
    match node.kind() {
        INTEGER => !text(node, source).ends_with(['j', 'J']),
        UNARY_OPERATOR => {
            let negative = node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
            (allow_negative || !negative)
                && node
                    .child_by_field_name(ARGUMENT_FIELD)
                    .is_some_and(|operand| is_integer(operand, source, allow_negative))
        }
        _ => false,
    }
}

// FURB192: `sorted(x)[0]` o `sorted(x)[-1]`, que son `min` y `max`. Avisa en el subíndice.
fn sorted_min_max(model: &Model<'_>, source: &[u8], call: Node<'_>, given: &Arguments<'_>) -> Option<Report> {
    let mut value = call;
    while let Some(parent) = value.parent().filter(|parent| parent.kind() == PARENTHESIZED) {
        value = parent;
    }
    let subscript = value.parent().filter(|parent| {
        parent.kind() == SUBSCRIPT
            && parent
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|inner| inner.id() == value.id())
    })?;
    let [_] = given.positional.as_slice() else {
        return None;
    };
    let keywords_allowed = given.keywords.iter().all(|keyword| {
        keyword.kind() == KEYWORD_ARGUMENT
            && keyword
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| ALLOWED_SORT_KEYWORDS.contains(&text(name, source)))
    });
    let mut cursor = subscript.walk();
    let indexes: Vec<Node<'_>> = subscript
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    let [index] = indexes.as_slice() else {
        return None;
    };
    let written = text(*index, source).replace(' ', "");
    let is_first = written == FIRST_INDEX;
    (keywords_allowed && (is_first || written == LAST_INDEX)).then(|| {
        let fix = min_max_function(given, is_first, source)
            .filter(|function| builtin_available(model, function))
            .map(|function| min_max_fix(subscript, given, function, source));
        Report::at(PythonRule::FURB192, subscript, "").with_fix(fix)
    })
}

// `min(x)` o `max(x)` según el índice y `reverse`, con el `key` si lo hay; inseguro, como en ruff, y
// sólo si el builtin no está tapado.
fn min_max_fix(subscript: Node<'_>, given: &Arguments<'_>, function: &str, source: &[u8]) -> Fix {
    let listed = given
        .positional
        .first()
        .map_or("", |listed| text(*listed, source));
    let replacement = keyword_named(given, KEY, source).map_or_else(
        || format!("{function}({listed})"),
        |key| format!("{function}({listed}, {})", text(key, source)),
    );
    Fix::replace(subscript, &replacement, source).marked_unsafe()
}

fn min_max_function(given: &Arguments<'_>, is_first: bool, source: &[u8]) -> Option<&'static str> {
    let reversed = match keyword_named(given, REVERSE, source)
        .and_then(|keyword| keyword.child_by_field_name(VALUE_FIELD))
    {
        None => false,
        Some(value) if value.kind() == TRUE => true,
        Some(value) if value.kind() == FALSE => false,
        Some(_) => return None,
    };
    Some(if is_first == reversed { MAX } else { MIN })
}

fn keyword_named<'tree>(given: &Arguments<'tree>, wanted: &str, source: &[u8]) -> Option<Node<'tree>> {
    given.keywords.iter().copied().find(|keyword| {
        keyword
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == wanted)
    })
}

// RUF057: `round` de un entero sin decimales negativos, que devuelve lo mismo.
fn is_unnecessary_round(given: &Arguments<'_>, source: &[u8]) -> bool {
    let Some(number) = given.positional.first().copied() else {
        return false;
    };
    let ndigits = keyword_value(given, NDIGITS, source).or_else(|| given.positional.get(1).copied());
    let digits_ok = ndigits.is_none_or(|digits| {
        unparenthesized(digits).kind() == NONE_LITERAL || is_integer(digits, source, false)
    });
    given.positional.len() <= 2 && is_integer(number, source, true) && digits_ok
}

// El arreglo de FURB166: el string sin el subíndice y la base en `0`; inseguro, como en ruff.
fn prefixed_base_fix(given: &Arguments<'_>, source: &[u8]) -> Option<Fix> {
    let [value, base] = given.positional.as_slice() else {
        return None;
    };
    let sliced = unparenthesized(*value);
    let string = sliced.child_by_field_name(VALUE_FIELD)?;
    let rewrite = |node: Node<'_>, replacement: &str| TextEdit {
        start: TextPoint::from_point(node.start_position()),
        end: TextPoint::from_point(node.end_position()),
        replacement: replacement.to_owned(),
    };
    let edits = vec![
        rewrite(sliced, text(string, source)),
        rewrite(*base, AUTOMATIC_BASE),
    ];
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

// FURB166: `int(s[2:], 16)` (o base 2 u 8), que es `int(s, 0)`.
fn is_prefix_stripped_int(given: &Arguments<'_>, source: &[u8]) -> bool {
    let [value, base] = given.positional.as_slice() else {
        return false;
    };
    let value = unparenthesized(*value);
    let starts_after_prefix = value.kind() == SUBSCRIPT
        && {
            let mut cursor = value.walk();
            let slices: Vec<Node<'_>> = value
                .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
                .collect();
            matches!(slices.as_slice(), [slice] if slice.kind() == SLICE && text(*slice, source).replace(' ', "") == "2:")
        };
    starts_after_prefix && given.keywords.is_empty() && PREFIXED_BASES.contains(&text(*base, source))
}

// FURB105: `print("")` o un `sep` que no separa nada.
fn is_unnecessary_print_argument(given: &Arguments<'_>, source: &[u8]) -> bool {
    if given
        .positional
        .iter()
        .any(|argument| argument.kind() == "list_splat")
        || given.positional.len() > 1
    {
        return false;
    }
    let has_sep = keyword_value(given, SEP, source).is_some();
    let empty_only = matches!(given.positional.as_slice(), [only] if is_string(*only)
        && text_value(*only, source).is_some_and(|value| value.is_empty()));
    has_sep || empty_only
}

// El `EmptyStringFix` de ruff con el separador fuera: la llamada vuelta a escribir sin los strings
// vacíos ni el `sep`; inseguro con comentarios o con un `sep` que no es un string ni `None`.
fn print_fix(call: Node<'_>, function: Node<'_>, given: &Arguments<'_>, source: &[u8]) -> Fix {
    let mut unsafe_separator = false;
    let mut parts: Vec<String> = given
        .positional
        .iter()
        .filter(|argument| !is_empty_string(**argument, source))
        .map(|argument| codegen::unparse(*argument, codegen::COMMA, source))
        .collect();
    for keyword in &given.keywords {
        let name = keyword
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source));
        if name == Some(SEP) {
            let value = keyword.child_by_field_name(VALUE_FIELD).map(unparenthesized);
            unsafe_separator |= !value.is_some_and(|value| is_string(value) || value.kind() == NONE_LITERAL);
            continue;
        }
        parts.push(codegen::unparse(*keyword, codegen::COMMA, source));
    }
    let replacement = format!(
        "{}({})",
        codegen::unparse(function, codegen::MAX, source),
        parts.join(", ")
    );
    let fix = Fix::replace(call, &replacement, source);
    if unsafe_separator || has_comment_within(call, call.start_byte(), call.end_byte()) {
        fix.marked_unsafe()
    } else {
        fix
    }
}

fn is_empty_string(argument: Node<'_>, source: &[u8]) -> bool {
    let argument = unparenthesized(argument);
    is_string(argument) && text_value(argument, source).is_some_and(|value| value.is_empty())
}

// B005: `strip(".txt")`, que saca caracteres sueltos y no el sufijo.
fn is_repeated_strip(function: Node<'_>, given: &Arguments<'_>, source: &[u8]) -> bool {
    let is_strip = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| STRIPS.contains(&text(attribute, source)));
    let [argument] = given.positional.as_slice() else {
        return false;
    };
    let Some(value) = is_string(*argument)
        .then(|| text_value(*argument, source))
        .flatten()
    else {
        return false;
    };
    let letters: Vec<char> = value.chars().collect();
    let repeated = letters.iter().enumerate().any(|(index, letter)| {
        letters
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == letter)
    });
    is_strip && given.keywords.is_empty() && letters.len() > 1 && repeated
}

// UP014: `Point = NamedTuple("Point", [("x", int)])`, que se escribe como clase.
fn is_functional_named_tuple(
    hints: Hints<'_, '_, '_>,
    call: Node<'_>,
    function: Node<'_>,
    given: &Arguments<'_>,
) -> bool {
    let assigned = call.parent().is_some_and(|parent| {
        parent.kind() == ASSIGNMENT
            && parent
                .child_by_field_name(RIGHT_FIELD)
                .is_some_and(|value| value.id() == call.id())
            && parent
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|target| target.kind() == IDENTIFIER)
    });
    let is_named_tuple =
        qualified_name(hints.model, function, hints.source).is_some_and(|name| is_typing(&name, NAMED_TUPLE));
    if !assigned || !is_named_tuple {
        return false;
    }
    match given.positional.as_slice() {
        [name] => is_string(*name),
        [name, fields] => {
            is_string(*name) && given.keywords.is_empty() && are_field_pairs(*fields, hints.source)
        }
        _ => false,
    }
}

fn are_field_pairs(fields: Node<'_>, source: &[u8]) -> bool {
    let fields = unparenthesized(fields);
    fields.kind() == LIST
        && named(fields)
            .into_iter()
            .filter(|field| field.kind() != COMMENT)
            .all(|field| {
                let parts: Vec<Node<'_>> = named(field)
                    .into_iter()
                    .filter(|part| part.kind() != COMMENT)
                    .collect();
                field.kind() == TUPLE
                    && matches!(parts.as_slice(), [name, _] if is_string(*name)
                        && text_value(*name, source).is_some_and(|value| is_identifier(&value)))
            })
}

fn is_identifier(value: &str) -> bool {
    let mut letters = value.chars();
    letters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && letters.all(|letter| letter.is_alphanumeric() || letter == '_')
}

// PLW0245: `super.metodo()` sin los paréntesis de `super()`, adentro de un método.
fn super_without_call(
    model: &Model<'_>,
    source: &[u8],
    call: Node<'_>,
    function: Node<'_>,
) -> Option<Report> {
    let object = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| function.kind() == ATTRIBUTE && object.kind() == IDENTIFIER)?;
    if !is_builtin_named(model, object, source, SUPER) {
        return None;
    }
    let method = std::iter::successors(call.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()))?;
    (method.kind() == FUNCTION && method_kind(method, source).is_some())
        .then(|| Report::at(PythonRule::PLW0245, object, ""))
}
