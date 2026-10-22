use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_receivers::Variables;
use crate::syntax::{named_children, one_based};

const INVOCATION: &str = "method_invocation";
const IDENTIFIER: &str = "identifier";
const BINARY: &str = "binary_expression";
const NULL: &str = "null_literal";
const NOT_EQUAL: &str = "!=";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERATOR_FIELD: &str = "operator";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const ASSERT_THAT: &str = "assertThat";
const IS_NOT_NULL: &str = "isNotNull";
const NON_NULL_CHECKS: &[&str] = &["assertNotNull", "requireNonNull", "notNull"];

// De dónde sale el primer objeto de una cadena: una variable con su tipo declarado, o una clase
// cuyo método estático se llama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainRoot {
    Instance(String),
    Class(String),
}

// Un paso de una cadena de llamadas: el método, la cantidad de argumentos, dónde está su nombre y
// si su resultado ya se chequeó contra null antes en el archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainStep {
    pub name: String,
    pub arity: usize,
    pub line: usize,
    pub column: usize,
    pub checked: bool,
}

// `respuesta.getBody().error()`: la raíz y los pasos en orden. Sólo interesan las de al menos dos
// pasos, donde el resultado de una llamada se usa para llamar a otra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub root: ChainRoot,
    pub steps: Vec<ChainStep>,
}

// Lo que se sabe del archivo hasta el punto que se recorre: lo que ya se chequeó contra null (por
// texto) y las llamadas que ya son parte de una cadena.
#[derive(Default)]
pub struct NullFacts {
    checked: HashSet<String>,
    seen: HashSet<usize>,
}

impl NullFacts {
    // Un chequeo vale dentro del método donde está: al entrar en otro se olvida.
    pub fn enter_method(&mut self) {
        self.checked.clear();
    }

    // Se llama en orden de recorrido: un chequeo cuenta para lo que viene después.
    pub fn observe(&mut self, node: Node<'_>, source: &[u8]) {
        if let Some(checked) = checked_expression(node, source) {
            self.checked.insert(compact(text(checked, source)));
        }
    }

    fn is_checked(&self, expression: Node<'_>, source: &[u8]) -> bool {
        self.checked.contains(&compact(text(expression, source)))
    }
}

// Lo que un nodo da por no nulo: `x != null`, `assertThat(x).isNotNull()`, `assertNotNull(x)`,
// `requireNonNull(x)`.
fn checked_expression<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if node.kind() == BINARY {
        let operator = node.child_by_field_name(OPERATOR_FIELD)?;
        let left = node.child_by_field_name(LEFT_FIELD)?;
        let right = node.child_by_field_name(RIGHT_FIELD)?;
        if operator.kind() != NOT_EQUAL {
            return None;
        }
        return match (left.kind(), right.kind()) {
            (NULL, _) => Some(right),
            (_, NULL) => Some(left),
            _ => None,
        };
    }
    if node.kind() != INVOCATION {
        return None;
    }
    let name = text(node.child_by_field_name(NAME_FIELD)?, source);
    if NON_NULL_CHECKS.contains(&name) {
        return first_argument(node);
    }
    let object = node.child_by_field_name(OBJECT_FIELD)?;
    let asserted = object.kind() == INVOCATION
        && object
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|inner| text(inner, source) == ASSERT_THAT);
    (name == IS_NOT_NULL && asserted)
        .then(|| first_argument(object))
        .flatten()
}

fn first_argument(invocation: Node<'_>) -> Option<Node<'_>> {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .find(|argument| !COMMENTS.contains(&argument.kind()))
}

fn compact(written: &str) -> String {
    written.split_whitespace().collect()
}

// La cadena que termina en esta llamada, si no es parte de otra más larga ya vista. Una variable
// asignada desde una llamada no se sigue: el código real la chequea con `if (x == null) return;` y
// eso pide flujo de datos, no texto.
pub fn chain_of(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
    facts: &mut NullFacts,
) -> Option<Chain> {
    if facts.seen.contains(&invocation.id()) {
        return None;
    }
    let (root, steps) = walk(invocation, source, variables, facts)?;
    (steps.len() > 1).then_some(Chain { root, steps })
}

// La cadena que da valor a una variable, de un paso o más (`manager.getCache(nombre)`).
pub fn origin_chain(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Option<Chain> {
    let (root, steps) = walk(invocation, source, variables, &mut NullFacts::default())?;
    Some(Chain { root, steps })
}

fn walk(
    invocation: Node<'_>,
    source: &[u8],
    variables: &Variables,
    facts: &mut NullFacts,
) -> Option<(ChainRoot, Vec<ChainStep>)> {
    let mut steps = Vec::new();
    let mut current = invocation;
    loop {
        facts.seen.insert(current.id());
        let name = current.child_by_field_name(NAME_FIELD)?;
        let start = name.start_position();
        steps.push(ChainStep {
            name: text(name, source).to_owned(),
            arity: argument_count(current),
            line: one_based(start.row),
            column: one_based(start.column),
            checked: facts.is_checked(current, source),
        });
        let object = current.child_by_field_name(OBJECT_FIELD)?;
        if object.kind() == INVOCATION {
            current = object;
            continue;
        }
        if object.kind() != IDENTIFIER {
            return None;
        }
        steps.reverse();
        let written = text(object, source);
        let root = match variables.get(written) {
            Some(Some(declared)) => ChainRoot::Instance(declared.clone()),
            None if written.starts_with(|first: char| first.is_ascii_uppercase()) => {
                ChainRoot::Class(written.to_owned())
            }
            Some(None) | None => return None,
        };
        return Some((root, steps));
    }
}

fn argument_count(invocation: Node<'_>) -> usize {
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .count()
}
