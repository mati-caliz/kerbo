use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java::text;
use super::java_receivers::{declared_by, simple_type_name};
use crate::syntax::named_children;

const LOCAL_DECLARATION: &str = "local_variable_declaration";
const CREATION: &str = "object_creation_expression";
const CLASS_BODY: &str = "class_body";
const ASSIGNMENT: &str = "assignment_expression";
const UPDATE: &str = "update_expression";
const IDENTIFIER: &str = "identifier";
const VALUE_FIELD: &str = "value";
const TYPE_FIELD: &str = "type";
const LEFT_FIELD: &str = "left";
const INFERRED: &str = "var";
const SCOPES: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "compact_constructor_declaration",
    "static_initializer",
];

// Las variables locales que siempre guardan un objeto recién creado de la clase con que se declaran
// (`final Contador contador = new Contador();`) y nunca se reasignan: el flujo de IntelliJ sabe el
// tipo exacto del receptor y resuelve la llamada al método de esa clase aunque se pueda redefinir.
// Cada nombre se mira dentro del método que lo declara: otro método puede tener una variable igual.
pub struct ExactReceivers {
    variables: HashSet<ScopedName>,
}

type ScopedName = (String, Option<usize>);

impl ExactReceivers {
    pub fn of(nodes: &[Node<'_>], source: &[u8]) -> Self {
        let mut declarations: HashMap<ScopedName, bool> = HashMap::new();
        for node in nodes {
            for (variable, written) in declared_by(*node) {
                let exact = constructs_declared_type(*node, variable, written, source);
                declarations
                    .entry(scoped(variable, source))
                    .and_modify(|all_exact| *all_exact &= exact)
                    .or_insert(exact);
            }
        }
        let reassigned = reassigned_names(nodes, source);
        let variables = declarations
            .into_iter()
            .filter(|(name, exact)| *exact && !reassigned.contains(name))
            .map(|(name, _)| name)
            .collect();
        Self { variables }
    }

    pub fn contains(&self, identifier: Node<'_>, source: &[u8]) -> bool {
        self.variables.contains(&scoped(identifier, source))
    }
}

fn scoped(identifier: Node<'_>, source: &[u8]) -> ScopedName {
    (text(identifier, source).to_owned(), method_of(identifier))
}

fn method_of(node: Node<'_>) -> Option<usize> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if SCOPES.contains(&ancestor.kind()) {
            return Some(ancestor.id());
        }
        current = ancestor.parent();
    }
    None
}

fn constructs_declared_type(
    owner: Node<'_>,
    variable: Node<'_>,
    written: Option<Node<'_>>,
    source: &[u8],
) -> bool {
    if owner.kind() != LOCAL_DECLARATION {
        return false;
    }
    let Some(created) = variable
        .parent()
        .and_then(|declarator| declarator.child_by_field_name(VALUE_FIELD))
        .filter(|value| value.kind() == CREATION)
        .filter(|creation| {
            !named_children(*creation)
                .iter()
                .any(|child| child.kind() == CLASS_BODY)
        })
        .and_then(|creation| creation.child_by_field_name(TYPE_FIELD))
    else {
        return false;
    };
    written.is_some_and(|declared| {
        let declared = text(declared, source);
        declared == INFERRED || simple_type_name(declared) == simple_type_name(text(created, source))
    })
}

fn reassigned_names(nodes: &[Node<'_>], source: &[u8]) -> HashSet<ScopedName> {
    nodes
        .iter()
        .filter_map(|node| match node.kind() {
            ASSIGNMENT => node.child_by_field_name(LEFT_FIELD),
            UPDATE => named_children(*node).first().copied(),
            _ => None,
        })
        .filter(|target| target.kind() == IDENTIFIER)
        .map(|target| scoped(target, source))
        .collect()
}
