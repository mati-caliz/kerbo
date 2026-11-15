use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::argument_count;
use crate::syntax::named_children;

const BINARY: &str = "binary_expression";
const NULL: &str = "null_literal";
const NOT_EQUAL: &str = "!=";
const OPERATOR_FIELD: &str = "operator";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const ASSERT_THAT: &str = "assertThat";
const IS_NOT_NULL: &str = "isNotNull";
const NON_NULL_CHECKS: &[&str] = &["assertNotNull", "requireNonNull", "notNull"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const ARGUMENTS_FIELD: &str = "arguments";
const INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";
const VALUE_FIELD: &str = "value";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const FIELD_ACCESS: &str = "field_access";
const IDENTIFIER: &str = "identifier";
const GETTER_PREFIXES: &[&str] = &["get", "is"];

// Una llamada sin argumentos sobre una cadena que tampoco los tiene: con argumentos en el medio,
// cada vez puede ser otro objeto.
pub(super) fn is_stable_call(call: Node<'_>) -> bool {
    let mut current = Some(call);
    while let Some(node) = current.filter(|node| node.kind() == INVOCATION) {
        if argument_count(node) != 0 {
            return false;
        }
        current = node.child_by_field_name(OBJECT_FIELD);
    }
    true
}

// Lo que un nodo da por no nulo: `x != null`, `assertThat(x).isNotNull()`, `assertNotNull(x)`,
// `requireNonNull(x)`.
pub(super) fn checked_expression<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if node.kind() == BINARY {
        let operator = node.child_by_field_name(OPERATOR_FIELD)?;
        let left = node.child_by_field_name(LEFT_FIELD)?;
        let right = node.child_by_field_name(RIGHT_FIELD)?;
        if operator.kind() != NOT_EQUAL {
            return None;
        }
        return match (left.kind(), right.kind()) {
            (NULL, _) => Some(right),
            (_, NULL) => Some(left),
            _ => None,
        };
    }
    if node.kind() != INVOCATION {
        return None;
    }
    let name = text(node.child_by_field_name(NAME_FIELD)?, source);
    if NON_NULL_CHECKS.contains(&name) {
        return first_argument(node);
    }
    let object = node.child_by_field_name(OBJECT_FIELD)?;
    let asserted = object.kind() == INVOCATION
        && object
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|inner| text(inner, source) == ASSERT_THAT);
    (name == IS_NOT_NULL && asserted)
        .then(|| first_argument(object))
        .flatten()
}

fn first_argument(invocation: Node<'_>) -> Option<Node<'_>> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .find(|argument| !COMMENTS.contains(&argument.kind()))
}

// IntelliJ recuerda el valor de un getter (`getX()`, `isX()`), no el de cualquier llamada sin
// argumentos: `slave.toComputer()` se vuelve a marcar aunque antes se haya desreferenciado.
pub(super) fn is_getter_name(name: &str) -> bool {
    GETTER_PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with(|first: char| first.is_ascii_uppercase()))
    })
}

// `T local = receptor.getX();`, con una llamada sin argumentos: la local y el texto de la llamada.
pub(super) fn getter_local(node: Node<'_>, source: &[u8]) -> Option<(String, String)> {
    let value = node.child_by_field_name(VALUE_FIELD).filter(|value| {
        node.kind() == VARIABLE_DECLARATOR
            && value.kind() == INVOCATION
            && is_stable_call(*value)
            && value
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| is_getter_name(text(name, source)))
    })?;
    let name = node.child_by_field_name(NAME_FIELD)?;
    Some((text(name, source).to_owned(), text(value, source).to_owned()))
}

// La local que se desreferencia en `local.metodo()` o `local.campo`: si salió de un getter, IntelliJ
// da por no nula la misma llamada después.
pub(super) fn dereferenced_local<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    if node.kind() != INVOCATION && node.kind() != FIELD_ACCESS {
        return None;
    }
    let object = node.child_by_field_name(OBJECT_FIELD)?;
    (object.kind() == IDENTIFIER).then(|| text(object, source))
}
