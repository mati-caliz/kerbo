use tree_sitter::Node;

use crate::lint::python::ast::{IDENTIFIER, named};

const CONSTRAINED_TYPE: &str = "constrained_type";
pub const TYPE_PARAMETER_DEFAULT: &str = "type_parameter_default";

// El nombre y las cotas de un parámetro de tipo. El default de PEP 696 es un hermano en el árbol y no
// nombra a ningún parámetro.
pub fn type_parameter_parts(parameter: Node<'_>) -> (Option<Node<'_>>, Vec<Node<'_>>) {
    if parameter.kind() == TYPE_PARAMETER_DEFAULT {
        return (None, Vec::new());
    }
    let inner = named(parameter).first().copied().unwrap_or(parameter);
    let parts = if inner.kind() == CONSTRAINED_TYPE {
        named(inner)
    } else {
        vec![inner]
    };
    let Some((first, bounds)) = parts.split_first() else {
        return (None, Vec::new());
    };
    let name = std::iter::successors(Some(*first), |node| named(*node).first().copied())
        .find(|node| node.kind() == IDENTIFIER);
    (name, bounds.to_vec())
}
