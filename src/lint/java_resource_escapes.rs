use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::argument_count;
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const LAMBDA: &str = "lambda_expression";
const CLASS_BODY: &str = "class_body";
const ARGUMENTS: &str = "argument_list";
const BODY_FIELD: &str = "body";
const CLOSING_WORDS: &[&str] = &["close", "cleanup"];
const ASSERT_THROWS: &[&str] = &["assertThrows", "assertThrowsExactly"];
const PARENTHESIZED: &str = "parenthesized_expression";
const OBJECT_FIELD: &str = "object";
const RESOURCE: &str = "resource";
const VARIABLE_DECLARATOR: &str = "variable_declarator";

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

// Las llamadas que se encadenan sobre lo que creó la llamada de adentro (`j.createWebClient()
// .withThrowExceptionOnFailingStatusCode(false).login(...)`), en orden, con la de más afuera. Si todas
// devuelven el mismo tipo, IntelliJ (`ResourceInspection.getParent`) sigue a la de más afuera: el
// índice lo decide con los tipos.
pub(super) fn consumed_by<'tree>(first: Node<'tree>, source: &[u8]) -> (Vec<(String, usize)>, Node<'tree>) {
    let mut consumers = Vec::new();
    let mut current = first;
    loop {
        if let Some(name) = current.child_by_field_name(NAME_FIELD) {
            consumers.push((text(name, source).to_owned(), argument_count(current)));
        }
        let Some(parent) = skip_parentheses_up(current) else {
            return (consumers, current);
        };
        let is_receiver = parent.kind() == INVOCATION
            && parent
                .child_by_field_name(OBJECT_FIELD)
                .is_some_and(|object| unparenthesized_id(object) == current.id());
        if !is_receiver {
            return (consumers, current);
        }
        current = parent;
    }
}

// Si lo que da la cadena queda en una variable que lo cuida: un `try (...)` o una local que escapa o
// se cierra. Por la cadena IntelliJ sólo busca la variable (`getVariable`): un `return` o un
// argumento al final no la salvan (`return j.createWebClient().withBasicCredentials(user)` se marca).
pub(super) fn keeps_the_value(expression: Node<'_>, source: &[u8], escaped: &Escaped<'_>) -> bool {
    let Some(parent) = skip_parentheses_up(expression) else {
        return false;
    };
    match parent.kind() {
        VARIABLE_DECLARATOR => {
            parent
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| escaped.contains(&(text(name, source), innermost_lambda(name))))
                || parent
                    .parent()
                    .is_some_and(|declaration| closed_in_block(declaration, source))
        }
        kind => kind == RESOURCE,
    }
}

// Las variables que escaparon, con la lambda más cercana donde escaparon.
pub(super) type Escaped<'source> = HashSet<(&'source str, Option<usize>)>;

pub(super) fn innermost_lambda(node: Node<'_>) -> Option<usize> {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| ancestor.kind() == LAMBDA)
        .map(|lambda| lambda.id())
}

fn skip_parentheses_up(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    Some(parent)
}

fn unparenthesized_id(node: Node<'_>) -> usize {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        let Some(inner) = named_children(current).into_iter().next() else {
            break;
        };
        current = inner;
    }
    current.id()
}
