use tree_sitter::Node;

use crate::lint::java::{
    IDENTIFIER, OBJECT_CREATION, SCOPED_TYPE_IDENTIFIER, TYPE_FIELD, qualified_name, text,
};
use crate::syntax::{descendants, named_children};

// Checkstyle toma el nombre del primer hijo del nodo que envuelve al `new`: sólo es el tipo cuando
// la expresión va sola (argumento, inicialización, return); en un ternario, un cast, paréntesis,
// una asignación o una cadena de llamadas toma otra cosa y la superclase queda sin registrar.
const UNREGISTERED_PARENTS: [&str; 11] = [
    "ternary_expression",
    "cast_expression",
    "parenthesized_expression",
    "method_invocation",
    "field_access",
    "assignment_expression",
    "binary_expression",
    "instanceof_expression",
    "array_access",
    "method_reference",
    OBJECT_CREATION,
];
const FIELD_ACCESS: &str = "field_access";
const GENERIC_TYPE: &str = "generic_type";
const NAME_SEPARATOR: &str = ".";
const NEW_KEYWORD: &str = "new";

pub fn super_name(creation: Node<'_>, source: &[u8]) -> Option<String> {
    let parent = creation.parent()?;
    if UNREGISTERED_PARENTS.contains(&parent.kind()) {
        return None;
    }
    let created = type_name(creation.child_by_field_name(TYPE_FIELD)?, source)?;
    match creation.child(0) {
        Some(qualifier) if qualifier.kind() != NEW_KEYWORD => {
            let outer = qualifier_name(qualifier, source)?;
            Some([outer.as_str(), created.as_str()].join(NAME_SEPARATOR))
        }
        _ => Some(created),
    }
}

pub fn type_name(kind: Node<'_>, source: &[u8]) -> Option<String> {
    let breaks_name = descendants(kind).into_iter().any(|node| {
        node.kind() == SCOPED_TYPE_IDENTIFIER
            && named_children(node)
                .iter()
                .any(|child| child.kind() == GENERIC_TYPE)
    });
    (!breaks_name).then(|| qualified_name(kind, source))
}

fn qualifier_name(qualifier: Node<'_>, source: &[u8]) -> Option<String> {
    match qualifier.kind() {
        OBJECT_CREATION => type_name(qualifier.child_by_field_name(TYPE_FIELD)?, source),
        IDENTIFIER => Some(text(qualifier, source).to_owned()),
        FIELD_ACCESS => Some(qualified_name(qualifier, source)),
        _ => None,
    }
}
