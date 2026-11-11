use tree_sitter::Node;

use super::java::text;
use super::java_null_inference::parameter_type_name;
use crate::syntax::named_children;

const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const TYPE_IDENTIFIER: &str = "type_identifier";
const TYPE_BOUND: &str = "type_bound";
const NULLABLE: &str = "Nullable";
const TYPE_FIELD: &str = "type";
const GENERIC_TYPE: &str = "generic_type";
const SCOPED_TYPE_IDENTIFIER: &str = "scoped_type_identifier";

// Si el tipo calificado del parámetro lleva `@Nullable` en su último tramo (`Spliterator.@Nullable
// OfInt`, `java.util.@Nullable Optional<T>`), donde la anotación no queda entre los modificadores.
pub fn has_nullable_qualified_type(name: Node<'_>, source: &[u8]) -> bool {
    let Some(written) = name
        .parent()
        .and_then(|parameter| parameter.child_by_field_name(TYPE_FIELD))
    else {
        return false;
    };
    let qualified = if written.kind() == GENERIC_TYPE {
        named_children(written).into_iter().next()
    } else {
        Some(written)
    };
    qualified.is_some_and(|scoped| {
        scoped.kind() == SCOPED_TYPE_IDENTIFIER && text(scoped, source).contains(NULLABLE)
    })
}

// Si el tipo del parámetro es una variable de tipo con límite `@Nullable` (`<T extends @Nullable
// Object>`): la declarada más cerca, en el método o en las clases que lo encierran. Bajo
// `@NullMarked` esa variable admite null.
pub fn has_nullable_type_variable(name: Node<'_>, source: &[u8]) -> bool {
    closest_bound_is_nullable(name, source).unwrap_or(false)
}

// Si el tipo del parámetro (o el de sus elementos, en un arreglo o varargs) es una variable de tipo
// del método o de las clases que lo encierran.
pub fn is_type_variable(name: Node<'_>, source: &[u8]) -> bool {
    closest_bound_is_nullable(name, source).is_some()
}

fn closest_bound_is_nullable(name: Node<'_>, source: &[u8]) -> Option<bool> {
    let written = parameter_type_name(name, source);
    let mut current = name.parent();
    while let Some(node) = current {
        if let Some(nullable) = declared_bound_is_nullable(node, &written, source) {
            return Some(nullable);
        }
        current = node.parent();
    }
    None
}

fn declared_bound_is_nullable(declaration: Node<'_>, written: &str, source: &[u8]) -> Option<bool> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == TYPE_PARAMETERS)
        .flat_map(named_children)
        .filter(|parameter| parameter.kind() == TYPE_PARAMETER)
        .find(|parameter| {
            named_children(*parameter)
                .into_iter()
                .any(|part| part.kind() == TYPE_IDENTIFIER && text(part, source) == written)
        })
        .map(|parameter| {
            named_children(parameter)
                .into_iter()
                .any(|part| part.kind() == TYPE_BOUND && text(part, source).contains(NULLABLE))
        })
}
