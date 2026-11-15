use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const LAMBDA: &str = "lambda_expression";
const CLASS_BODY: &str = "class_body";
const ARGUMENTS: &str = "argument_list";
const BODY_FIELD: &str = "body";
const CLOSING_WORDS: &[&str] = &["close", "cleanup"];
const ASSERT_THROWS: &[&str] = &["assertThrows", "assertThrowsExactly"];

// IntelliJ (`ResourceInspection.EscapeVisitor`) da por escapado un recurso guardado en una local si
// en el bloque que la declara hay cualquier llamada cuyo nombre contenga "close" o "cleanup", sin
// importar sobre qué (`serverSocket.isClosed()`, `reader.lines().onClose(...)`). No mira dentro de
// lambdas ni de clases anónimas.
pub(super) fn closed_in_block(declaration: Node<'_>, source: &[u8]) -> bool {
    let Some(block) = declaration.parent() else {
        return false;
    };
    let mut pending = vec![block];
    while let Some(node) = pending.pop() {
        if node.kind() == INVOCATION && calls_something_closing(node, source) {
            return true;
        }
        pending.extend(
            named_children(node)
                .into_iter()
                .filter(|child| child.kind() != LAMBDA && child.kind() != CLASS_BODY),
        );
    }
    false
}

fn calls_something_closing(invocation: Node<'_>, source: &[u8]) -> bool {
    invocation
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source).to_lowercase())
        .is_some_and(|name| CLOSING_WORDS.iter().any(|word| name.contains(word)))
}

// Lo que crea una lambda de `assertThrows(..., () -> Files.newReader(...))`: su interfaz no devuelve
// nada, así que el recurso se pierde.
pub(super) fn discarded_by_assert_throws<'tree>(lambda: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let call = lambda
        .parent()
        .filter(|arguments| arguments.kind() == ARGUMENTS)?
        .parent()
        .filter(|call| call.kind() == INVOCATION)?;
    let name = text(call.child_by_field_name(NAME_FIELD)?, source);
    if !ASSERT_THROWS.contains(&name) {
        return None;
    }
    lambda
        .child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == INVOCATION || body.kind() == CREATION)
}
