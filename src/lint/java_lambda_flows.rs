use tree_sitter::Node;

use super::java_null_flow::{NullFlow, null_flows};
use super::java_receivers::{StaticImports, Variables};
use crate::syntax::named_children;

const LAMBDA: &str = "lambda_expression";
pub const STATIC_INITIALIZER: &str = "static_initializer";
const BLOCK: &str = "block";
const BODY_FIELD: &str = "body";

// El bloque `static { ... }` se recorre igual, como un método sin parámetros.
// El flujo dentro del cuerpo de una lambda con bloque (`sesion.then(r -> { User u = buscar(); u.x(); })`),
// recorrido como un método aparte: sólo sigue las variables que declara ella, sin lo que se sabía
// afuera.
pub fn lambda_flows(node: Node<'_>, source: &[u8], context: (&Variables, &StaticImports)) -> Vec<NullFlow> {
    let body = match node.kind() {
        LAMBDA => node.child_by_field_name(BODY_FIELD),
        STATIC_INITIALIZER => named_children(node)
            .into_iter()
            .find(|child| child.kind() == BLOCK),
        _ => None,
    };
    body.filter(|body| body.kind() == BLOCK)
        .map(|body| null_flows(node, body, source, context))
        .unwrap_or_default()
}
