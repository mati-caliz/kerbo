use tree_sitter::Node;

use super::Mode;
use super::pep::is_typing_only;
use super::qualified::is_builtin_named;
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{PARENTHESIZED, TUPLE};
use crate::rules::PythonRule;

const TYPE_NODE: &str = "type";
const TYPE_BUILTIN: &str = "type";
const MINIMUM_TYPE_MEMBERS: usize = 2;

// PYI055 fuera de un string: sólo donde Python no evalúa la anotación (con el import del futuro, en
// un stub o adentro de `if TYPE_CHECKING:`).
pub fn redundant_type_union(
    hints: Hints<'_, '_, '_>,
    node: Node<'_>,
    mode: Mode,
    deferred_annotations: bool,
) -> Option<Report> {
    (has_redundant_type_members(hints, node, mode)
        && is_typing_only(node, deferred_annotations, hints.source))
    .then(|| Report::at(PythonRule::PYI055, node, ""))
}

// PYI055: una unión (la de afuera, no las anidadas) con más de un `type[...]`, que se juntan en uno.
// Quien llama decide si la unión está en un contexto de tipos: ruff no la marca donde se evalúa.
pub fn has_redundant_type_members(hints: Hints<'_, '_, '_>, node: Node<'_>, mode: Mode) -> bool {
    if matches!(node.kind(), TYPE_NODE | PARENTHESIZED)
        || !hints.is_union(node, mode)
        || hints.is_nested(node, mode, Hints::is_union)
    {
        return false;
    }
    let mut members = Vec::new();
    hints.union_members(node, mode, &mut members);
    members
        .iter()
        .filter(|member| is_type_member(hints, **member))
        .count()
        >= MINIMUM_TYPE_MEMBERS
}

// `type[X]` del builtin, con un solo argumento que no es una tupla.
fn is_type_member(hints: Hints<'_, '_, '_>, member: Node<'_>) -> bool {
    subscript_parts(member).is_some_and(|(value, slices)| {
        matches!(slices.as_slice(), [only] if unwrapped(*only).kind() != TUPLE)
            && is_builtin_named(hints.model, value, hints.source, TYPE_BUILTIN)
    })
}
