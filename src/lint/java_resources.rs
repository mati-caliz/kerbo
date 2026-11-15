use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::{Chain, origin_chain};
use super::java_receivers::Variables;
use super::java_resource_escapes::{
    Escaped, closed_in_block, consumed_by, discarded_by_assert_throws, innermost_lambda, keeps_the_value,
};
use crate::syntax::{descendants, named_children, one_based};

const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const GENERIC_TYPE: &str = "generic_type";
const TYPE_FIELD: &str = "type";
const ARGUMENTS_FIELD: &str = "arguments";
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

// Lo que crea el posible recurso: una llamada (la cadena que la produce) o un `new` de la clase
// que se nombra, como está escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceOrigin {
    Call(Box<Chain>),
    Construction(String),
}

// Una llamada o un `new` cuyo resultado, si es algo que se cierra, quedaría sin cerrar, y dónde
// está su nombre. El índice decide con los jars y el proyecto si lo que crea se cierra; si alguno
// de los `owners` (sus argumentos y lo que la recibe) también se cierra, IntelliJ entiende que el
// recurso sale de otro y no lo marca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCandidate {
    pub origin: ResourceOrigin,
    pub owners: Vec<ResourceOrigin>,
    pub receiver: Option<ResourceOrigin>,
    pub line: usize,
    pub column: usize,
    pub consumers: Vec<(String, usize)>,
    pub kept_at_end: bool,
}

// Calca dónde mira AutoCloseableResource: una llamada o un `new` que inicializa una variable local
// que nunca va a un `try (variable)`, nunca se devuelve tal cual, no se guarda en otro lado y nunca
// recibe `close()`; y una llamada o un `new` sobre cuyo resultado se llama a otro método
// (`fabrica().getValidator()`), que pierde lo que creó.
pub fn resource_candidates(body: Node<'_>, source: &[u8], variables: &Variables) -> Vec<ResourceCandidate> {
    let nodes = descendants(body);
    let escaped = escaped_names(&nodes, source);
    let mut candidates = Vec::new();
    for &node in &nodes {
        match node.kind() {
            LOCAL if !closed_in_block(node, source) => candidates.extend(
                kept_initializers(node, source, &escaped)
                    .into_iter()
                    .filter_map(|value| candidate(value, source, variables)),
            ),
            LAMBDA => candidates.extend(
                discarded_by_assert_throws(node, source)
                    .and_then(|created| candidate(created, source, variables)),
            ),
            INVOCATION if !is_close(node, source) => {
                if let Some(created) = node.child_by_field_name(OBJECT_FIELD) {
                    let (consumers, end) = consumed_by(node, source);
                    let kept_at_end = keeps_the_value(end, source, &escaped);
                    candidates.extend(
                        candidate(created, source, variables).map(|found| ResourceCandidate {
                            consumers,
                            kept_at_end,
                            ..found
                        }),
                    );
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
    escaped: &Escaped<'_>,
) -> Vec<Node<'tree>> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| {
            declarator
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| !escaped.contains(&(text(name, source), innermost_lambda(name))))
        })
        .filter_map(|declarator| declarator.child_by_field_name(VALUE_FIELD))
        .collect()
}

fn candidate(created: Node<'_>, source: &[u8], variables: &Variables) -> Option<ResourceCandidate> {
    match created.kind() {
        INVOCATION => call_candidate(created, source, variables),
        CREATION => construction_candidate(created, source, variables),
        _ => None,
    }
}

fn call_candidate(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Option<ResourceCandidate> {
    let name = invocation
        .child_by_field_name(NAME_FIELD)
        .filter(|name| !text(*name, source).starts_with(GETTER_PREFIX))?;
    let start = name.start_position();
    Some(ResourceCandidate {
        origin: ResourceOrigin::Call(Box::new(origin_chain(invocation, source, variables)?)),
        owners: owners(invocation, source, variables),
        receiver: invocation
            .child_by_field_name(OBJECT_FIELD)
            .and_then(|object| owner_origin(object, source, variables)),
        line: one_based(start.row),
        column: one_based(start.column),
        consumers: Vec::new(),
        kept_at_end: false,
    })
}

// IntelliJ marca la referencia a la clase del `new`; el tipo se guarda sin sus argumentos.
fn construction_candidate(
    creation: Node<'_>,
    source: &[u8],
    variables: &Variables,
) -> Option<ResourceCandidate> {
    let written = creation.child_by_field_name(TYPE_FIELD)?;
    let start = written.start_position();
    Some(ResourceCandidate {
        origin: ResourceOrigin::Construction(constructed_class(creation, source)?),
        owners: owners(creation, source, variables),
        receiver: None,
        line: one_based(start.row),
        column: one_based(start.column),
        consumers: Vec::new(),
        kept_at_end: false,
    })
}

fn constructed_class(creation: Node<'_>, source: &[u8]) -> Option<String> {
    let written = creation.child_by_field_name(TYPE_FIELD)?;
    let class = if written.kind() == GENERIC_TYPE {
        named_children(written).into_iter().next()?
    } else {
        written
    };
    Some(text(class, source).to_owned())
}

// Los argumentos de la llamada o el `new` de los que se sabe qué crean o con qué tipo se declaran.
fn owners(created: Node<'_>, source: &[u8], variables: &Variables) -> Vec<ResourceOrigin> {
    created
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|owner| owner_origin(owner, source, variables))
        .collect()
}

fn owner_origin(owner: Node<'_>, source: &[u8], variables: &Variables) -> Option<ResourceOrigin> {
    match owner.kind() {
        INVOCATION => {
            origin_chain(owner, source, variables).map(|chain| ResourceOrigin::Call(Box::new(chain)))
        }
        CREATION => constructed_class(owner, source).map(ResourceOrigin::Construction),
        IDENTIFIER => variables
            .get(text(owner, source))
            .cloned()
            .flatten()
            .map(ResourceOrigin::Construction),
        _ => None,
    }
}

fn is_close(invocation: Node<'_>, source: &[u8]) -> bool {
    invocation
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|called| text(called, source) == CLOSE)
}

// Las variables que se cierran o pasan a ser de otro: en un `try (variable)`, devueltas tal cual,
// asignadas a otra cosa, con un `close()`, o pasadas como argumento. Cada una con la lambda más
// cercana donde pasa: como IntelliJ no entra en lambdas, sólo cuenta para una variable declarada en
// esa misma lambda (o fuera de toda lambda).
fn escaped_names<'source>(nodes: &[Node<'_>], source: &'source [u8]) -> Escaped<'source> {
    let mut escaped = Escaped::new();
    for &node in nodes {
        let lambda = innermost_lambda(node);
        if node.kind() == ARGUMENTS {
            escaped.extend(
                named_children(node)
                    .into_iter()
                    .filter(|argument| argument.kind() == IDENTIFIER)
                    .map(|argument| (text(argument, source), lambda)),
            );
            continue;
        }
        let name = match node.kind() {
            RESOURCE | RETURN => single_child(node).filter(|child| child.kind() == IDENTIFIER),
            ASSIGNMENT => node
                .child_by_field_name(RIGHT_FIELD)
                .filter(|right| right.kind() == IDENTIFIER),
            INVOCATION if is_close(node, source) => node
                .child_by_field_name(OBJECT_FIELD)
                .filter(|object| object.kind() == IDENTIFIER),
            _ => None,
        };
        escaped.extend(name.map(|name| (text(name, source), lambda)));
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
