use tree_sitter::Node;

use super::java::text;
use super::java_call_roots::root_of;
use super::java_chains::{argument_kind, call_arguments};
use super::java_receivers::{Variables, simple_type_name};
use super::java_throws::{
    ArgumentKind, BODY_FIELD, CONSTRUCTOR_REFERENCE, CallRoot, LAMBDA, METHOD_REFERENCE, throw_points,
};
use crate::syntax::named_children;

const TYPE_KINDS: &[&str] = &[
    "type_identifier",
    "scoped_type_identifier",
    "generic_type",
    "array_type",
];
const IDENTIFIER: &str = "identifier";

// Los argumentos de una llamada, con lo que tiran las lambdas y las referencias a método: eso dice
// qué es el `E` de un `throws E` que la interfaz funcional le pasa al método.
pub(super) fn argument_kinds(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
) -> Vec<ArgumentKind> {
    call_arguments(invocation)
        .into_iter()
        .map(|argument| match argument.kind() {
            LAMBDA => lambda_kind(argument, source, variables),
            METHOD_REFERENCE => reference_kind(argument, source, variables),
            _ => argument_kind(argument, source),
        })
        .collect()
}

fn lambda_kind(lambda: Node<'_>, source: &[u8], variables: &Variables) -> ArgumentKind {
    lambda
        .child_by_field_name(BODY_FIELD)
        .map_or(ArgumentKind::Unknown, |body| {
            ArgumentKind::Lambda(throw_points(body, source, variables))
        })
}

// `Tipo::metodo`, `variable::metodo`, `this::metodo` y `Tipo::new`. Lo que no se sabe de dónde sale
// queda como desconocido.
fn reference_kind(reference: Node<'_>, source: &[u8], variables: &Variables) -> ArgumentKind {
    let Some(qualifier) = named_children(reference).into_iter().next() else {
        return ArgumentKind::Unknown;
    };
    let Some(name) = referenced_name(reference, source) else {
        return ArgumentKind::Unknown;
    };
    let root = if TYPE_KINDS.contains(&qualifier.kind()) {
        Some(CallRoot::Type(simple_type_name(text(qualifier, source))))
    } else {
        root_of(qualifier, source, variables)
    };
    root.map_or(ArgumentKind::Unknown, |root| ArgumentKind::Reference(root, name))
}

fn referenced_name(reference: Node<'_>, source: &[u8]) -> Option<String> {
    let last = reference.child(reference.child_count().checked_sub(1)?)?;
    match last.kind() {
        IDENTIFIER | CONSTRUCTOR_REFERENCE => Some(text(last, source).to_owned()),
        _ => None,
    }
}
