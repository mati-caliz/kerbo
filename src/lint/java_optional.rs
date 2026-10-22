use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const GET: &str = "get";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
// Lo que devuelve un `Optional` recién hecho, con su cantidad de argumentos: las búsquedas de un
// stream (`max` y `min` con su comparador) y `Optional.ofNullable`.
const OPTIONAL_PRODUCERS: &[(&str, usize)] = &[
    ("findFirst", 0),
    ("findAny", 0),
    ("max", 1),
    ("min", 1),
    ("ofNullable", 1),
];

// Calca OptionalGetWithoutIsPresent en el caso que no pide flujo de datos: `get()` encadenado
// directamente a lo que crea el `Optional` (`stream.filter(...).findFirst().get()`), donde no hay
// lugar para chequear si tiene valor.
pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    nodes
        .iter()
        .filter(|node| node.kind() == INVOCATION)
        .filter_map(|invocation| unchecked_get(*invocation, source))
        .collect()
}

fn unchecked_get(invocation: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let name = invocation.child_by_field_name(NAME_FIELD)?;
    if text(name, source) != GET || argument_count(invocation) != 0 {
        return None;
    }
    let creation = invocation
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == INVOCATION)?;
    let produced = text(creation.child_by_field_name(NAME_FIELD)?, source);
    OPTIONAL_PRODUCERS
        .contains(&(produced, argument_count(creation)))
        .then(|| {
            finding(
                LintKind::Java(JavaRule::OptionalGetWithoutIsPresent),
                name,
                produced,
                None,
            )
        })
}

fn argument_count(invocation: Node<'_>) -> usize {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .count()
}
