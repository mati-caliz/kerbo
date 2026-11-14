use tree_sitter::Node;

use super::java_receiver_arguments::{ArgumentNullness, receiver_arguments};
use crate::syntax::one_based;

const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const OBJECT_FIELD: &str = "object";

// Dónde empieza la llamada si lo que devuelve se usa para leer un campo (`getSubscriber().eventos`):
// Qodana marca ahí la desreferencia.
pub(super) fn field_read_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    let parent = invocation.parent()?;
    let reads_field = parent.kind() == FIELD_ACCESS
        && parent
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| object.id() == invocation.id());
    let start = invocation.start_position();
    reads_field.then(|| (one_based(start.row), one_based(start.column)))
}

// En qué lugares de sus argumentos de tipo se declara `@Nullable` la variable donde empieza la
// cadena (`Function<String, @Nullable Integer> function` en `function.apply(x).intValue()`): lo que
// devuelve el método en esa variable de la clase puede ser null.
pub(super) fn nullable_root_arguments(invocation: Node<'_>, source: &[u8]) -> Vec<usize> {
    let mut innermost = invocation;
    while let Some(object) = innermost
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == INVOCATION)
    {
        innermost = object;
    }
    receiver_arguments(innermost, source)
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, nullness)| **nullness == ArgumentNullness::Nullable)
        .map(|(position, _)| position)
        .collect()
}
