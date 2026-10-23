use tree_sitter::Node;

use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, CALL, COMPARISON, CONCATENATED_STRING, FUNCTION_FIELD, IDENTIFIER,
    PARENTHESIZED, STRING, is_bytes, named, string_prefix, text,
};

const BINARY: &str = "binary_operator";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const ELLIPSIS: &str = "ellipsis";
const TEMPLATE_PREFIX: char = 't';
const ARGUMENT_LIST: &str = "argument_list";
const ALWAYS_EFFECTS: [&str; 9] = [
    "named_expression",
    "await",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
    "subscript",
    "yield",
    CALL,
];
const SAFE_OPERANDS: [&str; 15] = [
    STRING,
    CONCATENATED_STRING,
    "integer",
    "float",
    "true",
    "false",
    "none",
    ELLIPSIS,
    "list",
    "tuple",
    "set",
    "dictionary",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
];
const ITERABLE_INITIALIZERS: [&str; 5] = ["set", "list", "tuple", "dict", "frozenset"];

// B018: una expresión suelta como sentencia que no hace nada. `is_builtin` dice si un nombre es el
// builtin en el alcance actual. Devuelve dónde avisar.
pub fn useless_expression<'tree>(
    statement: Node<'tree>,
    source: &[u8],
    is_builtin: &dyn Fn(Node<'_>) -> bool,
) -> Option<Node<'tree>> {
    let parts = named(statement);
    let value = match parts.as_slice() {
        [single] => unparenthesized(*single),
        _ => statement,
    };
    if parts.len() == 1 && is_exempt(value, source) {
        return None;
    }
    let effect = parts
        .iter()
        .any(|part| contains_effect(*part, source, is_builtin));
    (!effect || value.kind() == ATTRIBUTE).then_some(value)
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match named(current).as_slice() {
            [inner] => current = *inner,
            _ => return current,
        }
    }
    current
}

// Una comparación (B015), un string (docstrings y similares) o `...` no son B018.
fn is_exempt(value: Node<'_>, source: &[u8]) -> bool {
    match value.kind() {
        COMPARISON | ELLIPSIS => true,
        STRING => !is_bytes(value, source) && !is_template(value, source),
        CONCATENATED_STRING => named(value)
            .iter()
            .any(|part| !is_bytes(*part, source) && !is_template(*part, source)),
        _ => false,
    }
}

// `contains_effect` de ruff: alguna subexpresión puede tener efectos.
pub fn contains_effect(node: Node<'_>, source: &[u8], is_builtin: &dyn Fn(Node<'_>) -> bool) -> bool {
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if is_effect(current, source, is_builtin) {
            return true;
        }
        pending.extend(named(current));
    }
    false
}

fn is_effect(node: Node<'_>, source: &[u8], is_builtin: &dyn Fn(Node<'_>) -> bool) -> bool {
    match node.kind() {
        CALL => !is_empty_initializer(node, source, is_builtin),
        BINARY => !node
            .child_by_field_name(LEFT_FIELD)
            .zip(node.child_by_field_name(RIGHT_FIELD))
            .is_some_and(|(left, right)| is_safe_operand(left, source) && is_safe_operand(right, source)),
        kind => ALWAYS_EFFECTS.contains(&kind),
    }
}

fn is_safe_operand(operand: Node<'_>, source: &[u8]) -> bool {
    let operand = unparenthesized(operand);
    SAFE_OPERANDS.contains(&operand.kind()) && !has_template_part(operand, source)
}

// Una t-string (`t"…"`) no es un string para ruff: no se exime ni es un operando seguro.
fn is_template(string: Node<'_>, source: &[u8]) -> bool {
    string_prefix(string, source)
        .to_ascii_lowercase()
        .contains(TEMPLATE_PREFIX)
}

fn has_template_part(operand: Node<'_>, source: &[u8]) -> bool {
    match operand.kind() {
        STRING => is_template(operand, source),
        CONCATENATED_STRING => named(operand).iter().any(|part| is_template(*part, source)),
        _ => false,
    }
}

// `list()`, `dict()` y compañía sin argumentos no tienen efectos.
fn is_empty_initializer(call: Node<'_>, source: &[u8], is_builtin: &dyn Fn(Node<'_>) -> bool) -> bool {
    let empty = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| arguments.kind() == ARGUMENT_LIST && named(arguments).is_empty());
    empty
        && call.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
            function.kind() == IDENTIFIER
                && ITERABLE_INITIALIZERS.contains(&text(function, source))
                && is_builtin(function)
        })
}
