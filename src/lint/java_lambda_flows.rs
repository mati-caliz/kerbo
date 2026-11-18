use tree_sitter::Node;

use super::java_null_flow::{NullFlow, null_flows};
use super::java_receivers::{StaticImports, Variables};

const LAMBDA: &str = "lambda_expression";
const BLOCK: &str = "block";
const BODY_FIELD: &str = "body";

// El flujo dentro del cuerpo de una lambda con bloque (`sesion.then(r -> { User u = buscar(); u.x(); })`),
// recorrido como un método aparte: sólo sigue las variables que declara ella, sin lo que se sabía
// afuera.
pub fn lambda_flows(node: Node<'_>, source: &[u8], context: (&Variables, &StaticImports)) -> Vec<NullFlow> {
    if node.kind() != LAMBDA {
        return Vec::new();
    }
    node.child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == BLOCK)
        .map(|body| null_flows(node, body, source, context))
        .unwrap_or_default()
}
