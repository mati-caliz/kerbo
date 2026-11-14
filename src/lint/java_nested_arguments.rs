use tree_sitter::Node;

use super::java_null_inference::{CallContext, NullArgument, argument_of_call};
use crate::syntax::named_children;

const PARENTHESIZED: &str = "parenthesized_expression";
const ARGUMENT_LIST: &str = "argument_list";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Un `null` pasado a una llamada que es a su vez argumento de otra (`builder.put(immutableEntry(null,
// 1))`): si el parámetro del de adentro es una variable de tipo del método, IntelliJ la infiere del
// parámetro de afuera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedNull {
    pub inner: NullArgument,
    pub outer: NullArgument,
}

// La llamada de afuera que recibe a esta como argumento, con su posición.
pub(super) fn enclosing_argument(
    node: Node<'_>,
    source: &[u8],
    context: &CallContext<'_>,
) -> Option<NullArgument> {
    let written = outermost_parentheses(node);
    let list = written.parent().filter(|parent| parent.kind() == ARGUMENT_LIST)?;
    let index = named_children(list)
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .position(|argument| argument.id() == written.id())?;
    argument_of_call(list.parent()?, (index, node), source, context)
}

pub(super) fn outermost_parentheses(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while let Some(parent) = current.parent().filter(|parent| parent.kind() == PARENTHESIZED) {
        current = parent;
    }
    current
}
