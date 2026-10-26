use tree_sitter::Node;

use super::integer_calls::is_integer_literal;
use super::model::Model;
use super::qualified::qualified_name;
use crate::fix::Fix;
use crate::lint::python::ast::{
    ASSIGNMENT, CALL, EXPRESSION_STATEMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, PARENTHESIZED, RIGHT_FIELD,
    text, unparenthesized,
};
use crate::lint::python::fix_edits::has_comment_within;

const ARGUMENT_LIST: &str = "argument_list";
const RETURN: &str = "return_statement";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const ROUND: &str = "builtins.round";
const SAFE_INTEGER_RESULTS: [&str; 11] = [
    "builtins.len",
    "builtins.id",
    "builtins.hash",
    "builtins.ord",
    "builtins.int",
    "math.comb",
    "math.factorial",
    "math.gcd",
    "math.lcm",
    "math.isqrt",
    "math.perm",
];
// Lo que en ruff liga al menos como una llamada: átomos, llamadas, atributos y subíndices.
const TIGHT_KINDS: [&str; 22] = [
    "identifier",
    "integer",
    "float",
    "string",
    "concatenated_string",
    "true",
    "false",
    "none",
    "ellipsis",
    "list",
    "tuple",
    "dictionary",
    "set",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
    "call",
    "attribute",
    "subscript",
    "parenthesized_expression",
    "await",
];
const ALWAYS_PARENTHESIZED: [&str; 3] = ["named_expression", "yield", "expression_list"];
const LINE_FEED: char = '\n';
const DOT: u8 = b'.';
const OPEN_BRACE: u8 = b'{';

// El `unwrapped_call_argument` de ruff: el argumento de la llamada con los paréntesis que tenía, o con
// paréntesis nuevos si quedaría pegado a una palabra o a un `{`, si un entero quedaría delante de un
// `.`, si liga menos que una llamada fuera de un lugar donde no importa, o si es un `:=`, un `yield` o
// una tupla sin paréntesis.
pub fn unwrapped_call_argument(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> String {
    let written = text(argument, source);
    if argument.kind() == PARENTHESIZED {
        return written.to_owned();
    }
    let before = call
        .start_byte()
        .checked_sub(1)
        .and_then(|index| source.get(index));
    let after = source.get(call.end_byte());
    let glued_before = before
        .is_some_and(|byte| byte.is_ascii_alphabetic() || (*byte == OPEN_BRACE && written.starts_with('{')));
    let glued_after =
        after.is_some_and(|byte| byte.is_ascii_alphabetic() || (*byte == DOT && argument.kind() == INTEGER));
    let loose = !TIGHT_KINDS.contains(&argument.kind()) && !parent_allows_unparenthesized(call);
    let multiline = written.contains(LINE_FEED) && !TIGHT_KINDS.contains(&argument.kind());
    if glued_before || glued_after || loose || multiline || ALWAYS_PARENTHESIZED.contains(&argument.kind()) {
        format!("({written})")
    } else {
        written.to_owned()
    }
}

fn parent_allows_unparenthesized(call: Node<'_>) -> bool {
    let mut node = call;
    while let Some(parent) = node.parent().filter(|parent| parent.kind() == PARENTHESIZED) {
        node = parent;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        ARGUMENT_LIST | KEYWORD_ARGUMENT | EXPRESSION_STATEMENT | RETURN => true,
        ASSIGNMENT => parent
            .child_by_field_name(RIGHT_FIELD)
            .is_some_and(|value| value.id() == node.id()),
        _ => false,
    }
}

// RUF046: `int(x)` donde `x` ya es un entero. Seguro con un entero escrito o con lo que siempre
// devuelve un `int`; inseguro con los redondeos de `math`, con un `round` de algo que no está escrito,
// o con un comentario que se perdería.
pub fn cast_to_int_fix(model: &Model<'_>, call: Node<'_>, value: Node<'_>, source: &[u8]) -> Fix {
    let inner = unparenthesized(value);
    let safe = is_integer_literal(inner, source)
        || (inner.kind() == CALL && is_safe_integer_call(model, inner, source));
    let comments = has_comment_within(call, call.start_byte(), value.start_byte())
        || has_comment_within(call, value.end_byte(), call.end_byte());
    let fix = Fix::replace(call, &unwrapped_call_argument(call, value, source), source);
    if safe && !comments {
        fix
    } else {
        fix.marked_unsafe()
    }
}

fn is_safe_integer_call(model: &Model<'_>, call: Node<'_>, source: &[u8]) -> bool {
    let Some(name) = call
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|function| qualified_name(model, function, source))
    else {
        return false;
    };
    if SAFE_INTEGER_RESULTS.contains(&name.as_str()) {
        return true;
    }
    name == ROUND
        && super::builtin_calls::arguments(call)
            .positional
            .first()
            .is_some_and(|number| matches!(unparenthesized(*number).kind(), INTEGER | FLOAT))
}

// RUF057: `round(x)` de un entero, que queda `x`; seguro sólo si es un entero escrito.
pub fn unnecessary_round_fix(call: Node<'_>, number: Node<'_>, source: &[u8]) -> Fix {
    let fix = Fix::replace(call, &unwrapped_call_argument(call, number, source), source);
    let safe = is_integer_literal(unparenthesized(number), source)
        && !has_comment_within(call, call.start_byte(), call.end_byte());
    if safe { fix } else { fix.marked_unsafe() }
}
