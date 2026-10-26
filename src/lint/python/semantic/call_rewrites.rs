use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_argument};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, COMMENT, FUNCTION_FIELD, STRING, VALUE_FIELD, named, text, unparenthesized,
};
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::{Parentheses, commented_unsafe, remove_argument};
use crate::rules::PythonRule;

const DEFAULT_FACTORY: &str = "default_factory";
const MAP: &str = "map";
const EMPTY_LIST: &str = "[]";
const KEY: &str = "key";

// Los arreglos de RUF024, RUF026 y RUF058.
pub fn call_rewrite_fix(
    rule: PythonRule,
    call: Node<'_>,
    given: &Arguments<'_>,
    source: &[u8],
) -> Option<Fix> {
    match (rule, given.positional.as_slice()) {
        (PythonRule::RUF024, [keys, value]) => {
            let replacement = format!(
                "{{{KEY}: {} for {KEY} in {}}}",
                codegen::unparse(*value, codegen::COMMA, source),
                codegen::unparse(*keys, codegen::COMPREHENSION_TARGET, source)
            );
            Some(Fix::replace(call, &replacement, source).marked_unsafe())
        }
        (PythonRule::RUF026, _) => default_factory_fix(call, given, source),
        (PythonRule::RUF058, _) => starmap_fix(call, given, source),
        _ => None,
    }
}

// RUF026: `defaultdict(default_factory=list)` pasa a `defaultdict(list)`, con el valor adelante si
// hay más argumentos.
fn default_factory_fix(call: Node<'_>, given: &Arguments<'_>, source: &[u8]) -> Option<Fix> {
    let keyword = keyword_argument(given, DEFAULT_FACTORY, source)?;
    let value = text(keyword.child_by_field_name(VALUE_FIELD)?, source);
    let list = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let count = given.positional.len().saturating_add(given.keywords.len());
    if count == 1 {
        return Some(Fix::replace(keyword, value, source).marked_unsafe());
    }
    let first = named(list)
        .into_iter()
        .find(|argument| argument.kind() != COMMENT)?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(first.start_position()),
        end: TextPoint::from_point(first.start_position()),
        replacement: format!("{value}, "),
    }];
    edits.extend(remove_argument(keyword, list, Parentheses::Preserve, source).edits);
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

// RUF058: `starmap(f, zip(a, b))` pasa a `map(f, a, b)`, y `zip()` a `[]`; inseguro con comentarios.
fn starmap_fix(call: Node<'_>, given: &Arguments<'_>, source: &[u8]) -> Option<Fix> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let zip = unparenthesized(*given.positional.get(1)?);
    let zip_arguments = zip.child_by_field_name(ARGUMENTS_FIELD)?;
    let inner = text(zip_arguments, source);
    let inner = inner
        .get(1..inner.len().saturating_sub(1))
        .unwrap_or_default()
        .trim_end();
    let inner = inner.strip_suffix(',').unwrap_or(inner);
    let spread = if arguments(zip).positional.is_empty() {
        EMPTY_LIST
    } else {
        inner
    };
    let edits = vec![
        TextEdit {
            start: TextPoint::from_point(function.start_position()),
            end: TextPoint::from_point(function.end_position()),
            replacement: MAP.to_owned(),
        },
        TextEdit {
            start: TextPoint::from_point(given.positional.get(1)?.start_position()),
            end: TextPoint::from_point(given.positional.get(1)?.end_position()),
            replacement: spread.to_owned(),
        },
    ];
    Some(commented_unsafe(Fix::from_edits(source, edits), call, source))
}

// RUF030: `assert x, print("mensaje")` pasa a `assert x, "mensaje"` cuando el único argumento es un
// string; con otros argumentos ruff arma una f-string y eso todavía no está.
pub fn print_message_fix(message: Node<'_>, source: &[u8]) -> Option<Fix> {
    let given = arguments(message);
    let [only] = given.positional.as_slice() else {
        return None;
    };
    (given.keywords.is_empty() && unparenthesized(*only).kind() == STRING)
        .then(|| Fix::replace(message, text(unparenthesized(*only), source), source).marked_unsafe())
}

// PLW1507: `copy.copy(os.environ)` pasa a `os.environ.copy()`.
pub fn environ_copy_fix(call: Node<'_>, environ: Node<'_>, source: &[u8]) -> Fix {
    Fix::replace(call, &format!("{}.copy()", text(environ, source)), source).marked_unsafe()
}
