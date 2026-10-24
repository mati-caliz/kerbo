use tree_sitter::Node;

use super::Report;
use super::ast::{
    COMMENT, LIST, SUBSCRIPT, SUBSCRIPT_FIELD, TUPLE, VALUE_FIELD, fields, is_string, named, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const SLICE: &str = "slice";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const COMPLEX_SUFFIXES: [char; 2] = ['j', 'J'];
const INDEXABLE: [&str; 3] = [LIST, "list_comprehension", TUPLE];
const BOOLEANS: [&str; 2] = ["true", "false"];
const NONE: &str = "none";
// Lo que `CheckableExprType` de ruff sabe reconocer como índice, además de números y textos.
const OTHER_TYPES: [&str; 13] = [
    NONE,
    "true",
    "false",
    "ellipsis",
    LIST,
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "set",
    "dictionary",
    TUPLE,
    "generator_expression",
    "lambda",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Checkable {
    Integer,
    Boolean,
    None,
    Other,
}

// RUF016: una lista, tupla o texto escritos tal cual, indexados con algo que no es un entero ni un
// corte (o un corte con límites que no son enteros). Avisa en el índice o en el límite.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == SUBSCRIPT)
        .flat_map(|subscript| subscript_reports(*subscript, source))
        .collect()
}

fn subscript_reports(subscript: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(value) = subscript.child_by_field_name(VALUE_FIELD).map(unparenthesized) else {
        return Vec::new();
    };
    if !INDEXABLE.contains(&value.kind()) && !is_string(value) {
        return Vec::new();
    }
    let indexes = fields(subscript, SUBSCRIPT_FIELD);
    match indexes.as_slice() {
        [only] if only.kind() == SLICE => named(*only)
            .into_iter()
            .filter(|bound| bound.kind() != COMMENT)
            .filter(|bound| {
                classify(*bound, source).is_some_and(|kind| {
                    !matches!(kind, Checkable::Integer | Checkable::None | Checkable::Boolean)
                })
            })
            .map(|bound| Report::at(PythonRule::RUF016, bound, ""))
            .collect(),
        [only] => classify(unparenthesized(*only), source)
            .filter(|kind| !matches!(kind, Checkable::Integer | Checkable::Boolean))
            .map(|_| Report::at(PythonRule::RUF016, unparenthesized(*only), ""))
            .into_iter()
            .collect(),
        [first, ..] => vec![Report::at(PythonRule::RUF016, *first, "")],
        [] => Vec::new(),
    }
}

fn classify(index: Node<'_>, source: &[u8]) -> Option<Checkable> {
    let index = unparenthesized(index);
    match index.kind() {
        INTEGER if !text(index, source).ends_with(COMPLEX_SUFFIXES) => Some(Checkable::Integer),
        INTEGER | FLOAT | SLICE => Some(Checkable::Other),
        kind if BOOLEANS.contains(&kind) => Some(Checkable::Boolean),
        NONE => Some(Checkable::None),
        kind if OTHER_TYPES.contains(&kind) => Some(Checkable::Other),
        _ if is_string(index) => Some(Checkable::Other),
        _ => None,
    }
}
