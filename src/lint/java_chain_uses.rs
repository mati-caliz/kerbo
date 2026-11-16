use tree_sitter::Node;

use super::java::text;
use super::java_chains::{Chain, ChainRoot, call_arguments};
use super::java_null_inference::cast_type;
use super::java_receiver_arguments::{ArgumentNullness, receiver_arguments};
use super::java_receivers::{Variables, declared_simple_type, enclosing_types};
use crate::syntax::{named_children, one_based};

const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const FIELD_FIELD: &str = "field";
const CAST: &str = "cast_expression";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const PARENTHESIZED: &str = "parenthesized_expression";
const VALUE_FIELD: &str = "value";
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
fn thrown_at(invocation: Node<'_>) -> Option<(usize, usize)> {
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

pub(super) fn argument_count(invocation: Node<'_>) -> usize {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .count()
}

// `j.jenkins` o `r.jenkins.pluginManager`: los campos que se leen a partir de una variable de tipo
// conocido; el índice sabe de qué tipo es cada campo. Una variable que el archivo no declara puede
// ser un campo heredado.
pub(super) fn member_root(access: Node<'_>, source: &[u8], variables: &Variables) -> Option<ChainRoot> {
    let mut fields = Vec::new();
    let mut current = access;
    while current.kind() == FIELD_ACCESS {
        fields.push(text(current.child_by_field_name(FIELD_FIELD)?, source).to_owned());
        current = current.child_by_field_name(OBJECT_FIELD)?;
    }
    if current.kind() != IDENTIFIER {
        return None;
    }
    fields.reverse();
    let written = text(current, source);
    let owner = match variables.get(written) {
        Some(Some(declared)) => declared.clone(),
        Some(None) => declared_simple_type(current, written, source)?,
        None => return Some(inherited_root(current, source, fields)),
    };
    Some(ChainRoot::Member {
        owners: vec![owner],
        fields,
    })
}

// Una variable que el archivo no declara (`j` en una clase de test cuyo padre declara
// `protected JenkinsRule j`): el campo de una de las clases que encierran al uso o de sus padres.
pub(super) fn inherited_root(variable: Node<'_>, source: &[u8], following: Vec<String>) -> ChainRoot {
    let mut fields = vec![text(variable, source).to_owned()];
    fields.extend(following);
    ChainRoot::Member {
        owners: enclosing_types(variable, source),
        fields,
    }
}

// Una expresión sin llamadas que lee campos (`context.terminal`) o nombra uno heredado, como una
// cadena sin pasos: el índice dice de qué tipo es.
pub(super) fn field_chain(expression: Node<'_>, source: &[u8], variables: &Variables) -> Option<Chain> {
    let root = match expression.kind() {
        FIELD_ACCESS => member_root(expression, source, variables)?,
        IDENTIFIER => {
            let written = text(expression, source);
            let is_undeclared_variable = !variables.contains_key(written)
                && written.starts_with(|first: char| first.is_ascii_lowercase());
            is_undeclared_variable.then(|| inherited_root(expression, source, Vec::new()))?
        }
        _ => return None,
    };
    Some(Chain {
        root,
        steps: Vec::new(),
        dereferenced_at: None,
        passed_as: None,
        returned: None,
        nullable_root_arguments: Vec::new(),
        enclosing: Vec::new(),
        forwarded: Vec::new(),
        exact_root: None,
    })
}

// Dónde se desreferencia la llamada por el lugar que ocupa: se tira, se castea para llamar a otro
// método o se recorre con un `for`.
pub(super) fn used_by_parent_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    thrown_at(invocation)
        .or_else(|| cast_receiver_at(invocation))
        .or_else(|| iterated_at(invocation))
}

// Dónde empieza la llamada si es lo que recorre un `for (Archivo hijo : carpeta.listFiles())`.
fn iterated_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    let start = invocation.start_position();
    invocation
        .parent()
        .filter(|parent| parent.kind() == ENHANCED_FOR)
        .and_then(|parent| parent.child_by_field_name(VALUE_FIELD))
        .filter(|iterated| iterated.id() == invocation.id())
        .map(|_| (one_based(start.row), one_based(start.column)))
}

// Dónde está el nombre del método que se llama sobre la llamada casteada
// (`((FreeStyleProject) jenkins.getItem(nombre)).getBuilds()`): el casteo deja pasar el null.
fn cast_receiver_at(invocation: Node<'_>) -> Option<(usize, usize)> {
    let cast = invocation.parent().filter(|parent| {
        parent.kind() == CAST
            && parent
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| value.id() == invocation.id())
    })?;
    let parenthesized = cast.parent().filter(|parent| parent.kind() == PARENTHESIZED)?;
    let outer = parenthesized.parent().filter(|parent| {
        parent.kind() == INVOCATION
            && parent
                .child_by_field_name(OBJECT_FIELD)
                .is_some_and(|object| object.id() == parenthesized.id())
    })?;
    let start = outer.child_by_field_name(NAME_FIELD)?.start_position();
    Some((one_based(start.row), one_based(start.column)))
}

// El tipo de cada argumento cuando se sabe: una variable con su tipo declarado o un casteo.
pub(super) fn argument_types(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
) -> Vec<Option<String>> {
    call_arguments(invocation)
        .into_iter()
        .map(|argument| {
            if argument.kind() == IDENTIFIER {
                return variables.get(text(argument, source)).cloned().flatten();
            }
            cast_type(argument, source)
        })
        .collect()
}
