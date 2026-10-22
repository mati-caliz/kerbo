use tree_sitter::Node;

use super::java_constants::literal_value;
use crate::syntax::named_children;

const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const VOID: &str = "void_type";
const RETURN: &str = "return_statement";
const RETURN_BOUNDARIES: &[&str] = &["lambda_expression", "class_body"];
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// La forma del tipo que devuelve un método: sólo una referencia puede ser null.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnType {
    Void,
    Primitive,
    Reference,
}

// Qué devuelve un método: nada (void o un constructor), siempre la misma constante, o cosas que
// varían o no se pueden leer sin compilar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Returns {
    Nothing,
    Always(String),
    Varied,
}

pub fn return_type(declaration: Node<'_>) -> ReturnType {
    match declaration
        .child_by_field_name(TYPE_FIELD)
        .map(|written| written.kind())
    {
        None | Some(VOID) => ReturnType::Void,
        Some(kind) if PRIMITIVE_TYPES.contains(&kind) => ReturnType::Primitive,
        Some(_) => ReturnType::Reference,
    }
}

// Los `return` del método, sin entrar en lambdas ni en clases anónimas o locales.
pub fn returned(method: Node<'_>, source: &[u8]) -> Returns {
    if method
        .child_by_field_name(TYPE_FIELD)
        .is_none_or(|written| written.kind() == VOID)
    {
        return Returns::Nothing;
    }
    let Some(body) = method.child_by_field_name(BODY_FIELD) else {
        return Returns::Varied;
    };
    let mut values = Vec::new();
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if node.kind() == RETURN {
            values.push(
                named_children(node)
                    .into_iter()
                    .find(|child| !COMMENTS.contains(&child.kind()))
                    .and_then(|value| literal_value(value, source)),
            );
        } else if !RETURN_BOUNDARIES.contains(&node.kind()) {
            pending.extend(named_children(node));
        }
    }
    let mut all = values.into_iter();
    match all.next() {
        Some(Some(first)) if all.all(|value| value.as_deref() == Some(first.as_str())) => {
            Returns::Always(first)
        }
        _ => Returns::Varied,
    }
}
