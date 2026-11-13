use tree_sitter::Node;

use super::java::{NAME_FIELD, has_modifier, text};
use super::java_receivers::supertypes;
use crate::syntax::named_children;

const READ_RESOLVE: &str = "readResolve";
const READ_OBJECT: &str = "readObject";
const SERIALIZABLE: &str = "Serializable";
const OBJECT_INPUT_STREAM: &str = "ObjectInputStream";
const STATIC: &str = "static";
const METHOD_DECLARATION: &str = "method_declaration";
const CLASS_DECLARATION: &str = "class_declaration";
const CLASS_BODY: &str = "class_body";
const FORMAL_PARAMETER: &str = "formal_parameter";
const PARAMETERS_FIELD: &str = "parameters";
const TYPE_FIELD: &str = "type";
const QUALIFIER_SEPARATOR: char = '.';

// IntelliJ (`PlainDescriptor.isPossiblyNonInitialized`) da por posiblemente null todo campo de
// instancia en el `readResolve()` de una clase `Serializable` que no declara su `readObject`: la
// deserialización no pasa por el constructor. Devuelve el cuerpo de esa clase.
pub(super) fn resolved_without_construction<'tree>(
    method: Node<'tree>,
    source: &[u8],
) -> Option<Node<'tree>> {
    let is_read_resolve = method.kind() == METHOD_DECLARATION
        && !has_modifier(method, STATIC)
        && method
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == READ_RESOLVE)
        && parameter_types(method, source).is_empty();
    let body = method.parent().filter(|body| body.kind() == CLASS_BODY)?;
    let class = body.parent().filter(|class| class.kind() == CLASS_DECLARATION)?;
    let is_serializable = supertypes(class, source)
        .iter()
        .any(|supertype| supertype == SERIALIZABLE);
    (is_read_resolve && is_serializable && !declares_read_object(body, source)).then_some(body)
}

fn declares_read_object(body: Node<'_>, source: &[u8]) -> bool {
    named_children(body).into_iter().any(|member| {
        member.kind() == METHOD_DECLARATION
            && !has_modifier(member, STATIC)
            && member
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == READ_OBJECT)
            && parameter_types(member, source)
                .iter()
                .map(|written| written.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(written))
                .eq([OBJECT_INPUT_STREAM])
    })
}

fn parameter_types<'source>(method: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| parameter.kind() == FORMAL_PARAMETER)
        .filter_map(|parameter| parameter.child_by_field_name(TYPE_FIELD))
        .map(|written| text(written, source))
        .collect()
}
