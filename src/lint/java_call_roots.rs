use tree_sitter::Node;

use super::java::text;
use super::java_constructor_throws::super_type;
use super::java_receivers::{Variables, declared_arguments, enclosing_types};
use super::java_throws::{CallRoot, written_type};
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const ARRAY_ACCESS: &str = "array_access";
const PARENTHESIZED: &str = "parenthesized_expression";
const CAST: &str = "cast_expression";
const CREATION: &str = "object_creation_expression";
const STRING_LITERAL: &str = "string_literal";
const CLASS_LITERAL: &str = "class_literal";
const CLASS_TYPE: &str = "Class";
const THIS: &str = "this";
const SUPER: &str = "super";
const STRING_TYPE: &str = "String";
const TYPE_FIELD: &str = "type";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const ARRAY_FIELD: &str = "array";
const QUALIFIER_SEPARATOR: char = '.';

// Los argumentos de tipo de la variable dicen qué tira un método heredado de un genérico y por qué
// tipo sigue la cadena. Una clase (`Clase.metodo()`) no es una variable: buscar su declaración
// recorrería el archivo entero en cada llamada estática.
pub(super) fn with_arguments(
    root: CallRoot,
    object: Node<'_>,
    source: &[u8],
    variables: &Variables,
) -> CallRoot {
    let name = text(object, source);
    match root {
        CallRoot::Type(declared)
            if object.kind() == IDENTIFIER && matches!(variables.get(name), Some(Some(_))) =>
        {
            match declared_arguments(object, name, source) {
                Some(arguments) => CallRoot::Generic(declared, arguments),
                None => CallRoot::Type(declared),
            }
        }
        other @ (CallRoot::Scope(_) | CallRoot::Type(_) | CallRoot::Generic(..)) => other,
    }
}

// `(Tipo) valor` es un `Tipo`, y `arreglo[i]` tiene el tipo con que se declaró el arreglo: las
// variables guardan el tipo sin los corchetes.
pub(super) fn root_of(object: Node<'_>, source: &[u8], variables: &Variables) -> Option<CallRoot> {
    match object.kind() {
        THIS => Some(CallRoot::Scope(enclosing_types(object, source))),
        SUPER => super_type(object, source).map(CallRoot::Type),
        IDENTIFIER => identifier_root(text(object, source), variables),
        STRING_LITERAL => Some(CallRoot::Type(STRING_TYPE.to_owned())),
        CLASS_LITERAL => Some(CallRoot::Type(CLASS_TYPE.to_owned())),
        CREATION | CAST => object
            .child_by_field_name(TYPE_FIELD)
            .map(|written| CallRoot::Type(written_type(text(written, source)))),
        FIELD_ACCESS => field_root(object, source, variables),
        ARRAY_ACCESS => object
            .child_by_field_name(ARRAY_FIELD)
            .filter(|array| array.kind() == IDENTIFIER)
            .and_then(|array| identifier_root(text(array, source), variables)),
        PARENTHESIZED => named_children(object)
            .first()
            .and_then(|inner| root_of(*inner, source, variables)),
        _ => None,
    }
}

pub(super) fn identifier_root(name: &str, variables: &Variables) -> Option<CallRoot> {
    match variables.get(name) {
        Some(Some(declared)) => Some(CallRoot::Type(declared.clone())),
        None if name.starts_with(|first: char| first.is_ascii_uppercase()) => {
            Some(CallRoot::Type(name.to_owned()))
        }
        Some(None) | None => None,
    }
}

// `this.campo` tiene el tipo del campo, y `a.b.Clase` o `Afuera.Adentro` son una clase.
fn field_root(access: Node<'_>, source: &[u8], variables: &Variables) -> Option<CallRoot> {
    if access
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|owner| owner.kind() == THIS)
    {
        let field = access.child_by_field_name(FIELD_FIELD)?;
        return identifier_root(text(field, source), variables);
    }
    let written = text(access, source);
    written
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .is_some_and(|last| last.starts_with(|first: char| first.is_ascii_uppercase()))
        .then(|| CallRoot::Type(written.to_owned()))
}
