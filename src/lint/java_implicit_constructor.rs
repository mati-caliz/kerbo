use tree_sitter::Node;

use super::java::{CONSTRUCTOR, NAME_FIELD, text};
use super::java_calls::{Arity, Body, Callable, ReturnType, Returns, Visibility};
use super::java_owner::owner_of;
use crate::syntax::descendants;

const BODY_FIELD: &str = "body";

// Un enum sin constructor escrito tiene uno implícito sin parámetros: no se marca nada, pero
// cuenta para saber si una constante con argumentos va a otro.
pub fn implicit_enum_constructor<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    let name_node = declaration.child_by_field_name(NAME_FIELD)?;
    let declares_constructor = descendants(body).iter().any(|node| node.kind() == CONSTRUCTOR);
    (!declares_constructor).then(|| Callable {
        name: text(name_node, source).to_owned(),
        name_node,
        parameters: Vec::new(),
        is_constructor: true,
        visibility: Visibility::Private,
        overrides: false,
        body: Body::Abstract,
        arity: Arity::Fixed,
        returns: Returns::Nothing,
        return_type: ReturnType::Void,
        returned: String::new(),
        returned_arguments: Vec::new(),
        annotations: Vec::new(),
        owner: owner_of(declaration, source),
    })
}
