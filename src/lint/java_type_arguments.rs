use tree_sitter::Node;

use super::java::text;
use super::java_null_inference::parameter_type;
use super::java_type_variables::{method_variable_admits_null, type_parameter_admits_null};
use crate::syntax::named_children;

const GENERIC_TYPE: &str = "generic_type";
const TYPE_ARGUMENTS: &str = "type_arguments";
const ANNOTATED_TYPE: &str = "annotated_type";
const WILDCARD: &str = "wildcard";
const TYPE_IDENTIFIER: &str = "type_identifier";
const EXTENDS: &str = "extends";
const NULLABLE_ANNOTATIONS: &[&str] = &["@Nullable", "@CheckForNull"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const METHODS: &[&str] = &["method_declaration", "constructor_declaration"];

// Lo que se sabe de nulos de un argumento de tipo escrito (`Function<? super T, ? extends R>`):
// un tipo concreto, uno anotado `@Nullable` (o una variable con límite `@Nullable`), una variable
// del método que lo declara, otra variable de tipo (con su nombre), o un comodín sin `extends`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeArgument {
    Concrete,
    Nullable,
    MethodVariable,
    OtherVariable(String),
    Wildcard,
}

// Los argumentos de tipo de un tipo escrito, clasificados desde el lugar donde está escrito; el
// método, si lo hay, es el que declara las variables que cuentan como del método.
pub fn type_arguments_of(written: Node<'_>, source: &[u8]) -> Vec<TypeArgument> {
    let method =
        std::iter::successors(written.parent(), Node::parent).find(|node| METHODS.contains(&node.kind()));
    generic_type(written)
        .and_then(|generic| {
            named_children(generic)
                .into_iter()
                .find(|child| child.kind() == TYPE_ARGUMENTS)
        })
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .map(|argument| classify(argument, method, source))
        .collect()
}

// Los argumentos de tipo del tipo de un parámetro, a partir de su nombre.
pub fn parameter_type_arguments(name: Node<'_>, source: &[u8]) -> Vec<TypeArgument> {
    parameter_type(name)
        .map(|written| type_arguments_of(written, source))
        .unwrap_or_default()
}

fn generic_type(written: Node<'_>) -> Option<Node<'_>> {
    match written.kind() {
        GENERIC_TYPE => Some(written),
        ANNOTATED_TYPE => named_children(written).into_iter().find_map(generic_type),
        _ => None,
    }
}

fn classify(argument: Node<'_>, method: Option<Node<'_>>, source: &[u8]) -> TypeArgument {
    let written = text(argument, source);
    if NULLABLE_ANNOTATIONS
        .iter()
        .any(|annotation| written.contains(annotation))
    {
        return TypeArgument::Nullable;
    }
    match argument.kind() {
        WILDCARD => {
            let mut cursor = argument.walk();
            let bounded_above = argument
                .children(&mut cursor)
                .any(|child| child.kind() == EXTENDS);
            match named_children(argument).last() {
                Some(bound) if bounded_above => classify(*bound, method, source),
                _ => TypeArgument::Wildcard,
            }
        }
        TYPE_IDENTIFIER => variable_kind(argument, written, method, source),
        _ => TypeArgument::Concrete,
    }
}

fn variable_kind(argument: Node<'_>, written: &str, method: Option<Node<'_>>, source: &[u8]) -> TypeArgument {
    if let Some(admits_null) = method.and_then(|method| method_variable_admits_null(method, written, source))
    {
        return if admits_null {
            TypeArgument::Nullable
        } else {
            TypeArgument::MethodVariable
        };
    }
    match type_parameter_admits_null(argument, written, source) {
        Some(true) => TypeArgument::Nullable,
        Some(false) => TypeArgument::OtherVariable(written.to_owned()),
        None => TypeArgument::Concrete,
    }
}
