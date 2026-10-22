use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::{Chain, origin_chain};
use super::java_receivers::Variables;
use crate::syntax::{descendants, named_children, one_based};

const INVOCATION: &str = "method_invocation";
const IDENTIFIER: &str = "identifier";
const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const RESOURCE: &str = "resource";
const RETURN: &str = "return_statement";
const ASSIGNMENT: &str = "assignment_expression";
const OBJECT_FIELD: &str = "object";
const VALUE_FIELD: &str = "value";
const RIGHT_FIELD: &str = "right";
const CLOSE: &str = "close";
const GETTER_PREFIX: &str = "get";
const LAMBDA: &str = "lambda_expression";
const ARGUMENTS: &str = "argument_list";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Una llamada cuyo resultado, si es algo que se cierra, quedaría sin cerrar: la cadena que la
// produce y dónde está su nombre. El índice decide con los jars si lo que devuelve se cierra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCandidate {
    pub origin: Chain,
    pub line: usize,
    pub column: usize,
}

// Calca dónde mira AutoCloseableResource: una llamada que inicializa una variable local que nunca
// va a un `try (variable)`, nunca se devuelve tal cual, no se guarda en otro lado y nunca recibe
// `close()`; y una llamada sobre cuyo resultado se llama a otra (`fabrica().getValidator()`), que
// pierde lo que creó.
pub fn resource_candidates(body: Node<'_>, source: &[u8], variables: &Variables) -> Vec<ResourceCandidate> {
    let nodes = descendants(body);
    let escaped = escaped_names(&nodes, source);
    let mut candidates = Vec::new();
    for &node in &nodes {
        match node.kind() {
            LOCAL => candidates.extend(
                kept_initializers(node, source, &escaped)
                    .into_iter()
                    .filter_map(|value| candidate(value, source, variables)),
            ),
            INVOCATION => {
                if let Some(created) = node
                    .child_by_field_name(OBJECT_FIELD)
                    .filter(|object| object.kind() == INVOCATION)
                {
                    candidates.extend(candidate(created, source, variables));
                }
            }
            _ => {}
        }
    }
    candidates
}

// Lo que devuelve un `get...` (`response.getWriter()`, `factory.getConnection()`) es de quien lo da:
// el contenedor o el pool lo cierran, como entiende IntelliJ.
// Las llamadas que inicializan variables locales que no se cierran ni pasan a ser de otro.
fn kept_initializers<'tree>(
    declaration: Node<'tree>,
    source: &[u8],
    escaped: &HashSet<&str>,
) -> Vec<Node<'tree>> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| {
            declarator
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| !escaped.contains(text(name, source)))
        })
        .filter_map(|declarator| declarator.child_by_field_name(VALUE_FIELD))
        .filter(|value| value.kind() == INVOCATION)
        .collect()
}

fn candidate(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Option<ResourceCandidate> {
    let name = invocation
        .child_by_field_name(NAME_FIELD)
        .filter(|name| !text(*name, source).starts_with(GETTER_PREFIX))?;
    let start = name.start_position();
    Some(ResourceCandidate {
        origin: origin_chain(invocation, source, variables)?,
        line: one_based(start.row),
        column: one_based(start.column),
    })
}

// Las variables que se cierran o pasan a ser de otro: en un `try (variable)`, devueltas tal cual,
// asignadas a otra cosa, con un `close()`, o pasadas como argumento (fuera de una lambda, que corre
// después).
fn escaped_names<'source>(nodes: &[Node<'_>], source: &'source [u8]) -> HashSet<&'source str> {
    let mut escaped = HashSet::new();
    let lambdas: Vec<(usize, usize)> = nodes
        .iter()
        .filter(|node| node.kind() == LAMBDA)
        .map(|lambda| (lambda.start_byte(), lambda.end_byte()))
        .collect();
    let in_lambda = |node: Node<'_>| {
        lambdas
            .iter()
            .any(|(start, end)| *start <= node.start_byte() && node.end_byte() <= *end)
    };
    for &node in nodes {
        if node.kind() == ARGUMENTS && !in_lambda(node) {
            escaped.extend(
                named_children(node)
                    .into_iter()
                    .filter(|argument| argument.kind() == IDENTIFIER)
                    .map(|argument| text(argument, source)),
            );
            continue;
        }
        let name = match node.kind() {
            RESOURCE => single_child(node).filter(|child| child.kind() == IDENTIFIER),
            RETURN if !in_lambda(node) => single_child(node).filter(|child| child.kind() == IDENTIFIER),
            ASSIGNMENT => node
                .child_by_field_name(RIGHT_FIELD)
                .filter(|right| right.kind() == IDENTIFIER),
            INVOCATION => node
                .child_by_field_name(NAME_FIELD)
                .filter(|called| text(*called, source) == CLOSE)
                .and_then(|_| node.child_by_field_name(OBJECT_FIELD))
                .filter(|object| object.kind() == IDENTIFIER),
            _ => None,
        };
        escaped.extend(name.map(|name| text(name, source)));
    }
    escaped
}

fn single_child(node: Node<'_>) -> Option<Node<'_>> {
    let children: Vec<Node<'_>> = named_children(node)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()))
        .collect();
    match children.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}
