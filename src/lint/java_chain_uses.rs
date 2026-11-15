use tree_sitter::Node;

use super::java::text;
use super::java_receiver_arguments::{ArgumentNullness, receiver_arguments};
use crate::syntax::{named_children, one_based};

const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const THROW: &str = "throw_statement";
const OBJECT_FIELD: &str = "object";
const NAME_FIELD: &str = "name";
const CONDITION_FIELD: &str = "condition";
const IDENTIFIER: &str = "identifier";
const GUARDING_STATEMENTS: &[&str] = &["while_statement", "if_statement"];
const EMPTY_SENSITIVE: &[&str] = &["poll", "peek"];
const NOT_EMPTY_SUFFIX: &str = ".isEmpty()";
const NEGATION: &str = "!";
const ARGUMENTS_FIELD: &str = "arguments";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const CLASS_LITERAL: &str = "class_literal";
const STRING_LITERAL: &str = "string_literal";
const QUOTE: char = '"';

// Dónde empieza la llamada si lo que devuelve se usa para leer un campo (`getSubscriber().eventos`):
// Qodana marca ahí la desreferencia.
pub(super) fn field_read_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    let parent = invocation.parent()?;
    let reads_field = parent.kind() == FIELD_ACCESS
        && parent
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| object.id() == invocation.id());
    let start = invocation.start_position();
    reads_field.then(|| (one_based(start.row), one_based(start.column)))
}

// Dónde empieza la llamada si se tira lo que devuelve (`throw Throwables.rethrowAsException(e)`):
// tirar null es una desreferencia.
pub(super) fn thrown_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    let start = invocation.start_position();
    invocation
        .parent()
        .is_some_and(|parent| parent.kind() == THROW)
        .then(|| (one_based(start.row), one_based(start.column)))
}

// En qué lugares de sus argumentos de tipo se declara `@Nullable` la variable donde empieza la
// cadena (`Function<String, @Nullable Integer> function` en `function.apply(x).intValue()`): lo que
// devuelve el método en esa variable de la clase puede ser null.
pub(super) fn nullable_root_arguments(invocation: Node<'_>, source: &[u8]) -> Vec<usize> {
    let mut innermost = invocation;
    while let Some(object) = innermost
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == INVOCATION)
    {
        innermost = object;
    }
    receiver_arguments(innermost, source)
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, nullness)| **nullness == ArgumentNullness::Nullable)
        .map(|(position, _)| position)
        .collect()
}

// Si es un `poll()` o `peek()` de una colección que un `while` o un `if` que lo encierra da por no
// vacía (`while (!cola.isEmpty()) cola.poll().run();`): IntelliJ sigue el tamaño y no avisa.
pub(super) fn guarded_by_non_empty(invocation: Node<'_>, source: &[u8]) -> bool {
    let is_sensitive = invocation
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| EMPTY_SENSITIVE.contains(&text(name, source)));
    let Some(receiver) = invocation
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| is_sensitive && object.kind() == IDENTIFIER)
    else {
        return false;
    };
    let guard = format!("{NEGATION}{}{NOT_EMPTY_SUFFIX}", text(receiver, source));
    std::iter::successors(invocation.parent(), Node::parent)
        .filter(|node| GUARDING_STATEMENTS.contains(&node.kind()))
        .filter_map(|statement| statement.child_by_field_name(CONDITION_FIELD))
        .any(|condition| {
            let written: String = text(condition, source).split_whitespace().collect();
            written.contains(&guard)
        })
}

// Las posiciones de los argumentos que son un literal de clase (`Long.class`).
pub(super) fn class_arguments(invocation: Node<'_>) -> Vec<usize> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .enumerate()
        .filter(|(_, argument)| argument.kind() == CLASS_LITERAL)
        .map(|(position, _)| position)
        .collect()
}

// Lo que dicen los argumentos que son un texto literal (`System.getProperty("java.version")`).
pub(super) fn text_arguments(invocation: Node<'_>, source: &[u8]) -> Vec<String> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() == STRING_LITERAL)
        .map(|literal| text(literal, source).trim_matches(QUOTE).to_owned())
        .collect()
}
