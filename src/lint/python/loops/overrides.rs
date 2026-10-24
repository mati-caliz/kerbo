use tree_sitter::Node;

use super::names::{NameRole, name_role};
use crate::lint::python::Report;
use crate::lint::python::ast::{IDENTIFIER, LAMBDA, LEFT_FIELD, RIGHT_FIELD, named, text};
use crate::rules::PythonRule;

const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_clause";
const COMPREHENSIONS: [&str; 4] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];
const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const DEFAULTED_PARAMETERS: [&str; 2] = ["default_parameter", "typed_default_parameter"];
const NAME_FIELD: &str = "name";
const TYPED_PARAMETER: &str = "typed_parameter";

// B020: `for item in item:` (o un nombre del destino que también está en lo que se recorre), que
// pisa el iterable con cada elemento. Avisa en la última aparición del nombre en el destino.
pub fn loop_variable_overrides(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == FOR)
        .flat_map(|statement| {
            let mut targets = Vec::new();
            let mut iterated = Vec::new();
            if let Some(target) = statement.child_by_field_name(LEFT_FIELD) {
                collect_names(target, source, &mut targets);
            }
            if let Some(iterable) = statement.child_by_field_name(RIGHT_FIELD) {
                collect_names(iterable, source, &mut iterated);
            }
            targets
                .into_iter()
                .filter(|(name, _)| iterated.iter().any(|(other, _)| other == name))
                .map(|(_, node)| Report::at(PythonRule::B020, node, ""))
                .collect::<Vec<_>>()
        })
        .collect()
}

// El `NameFinder` de ruff: los nombres de la expresión, sin entrar a lo que produce una comprensión
// (sólo a lo que recorre) y sin los parámetros de una lambda. Un nombre repetido queda con su
// última aparición.
fn collect_names<'tree, 'source>(
    node: Node<'tree>,
    source: &'source [u8],
    names: &mut Vec<(&'source str, Node<'tree>)>,
) {
    match node.kind() {
        IDENTIFIER => {
            if name_role(node) != NameRole::Other {
                let name = text(node, source);
                names.retain(|(existing, _)| *existing != name);
                names.push((name, node));
            }
        }
        kind if COMPREHENSIONS.contains(&kind) => {
            for iterable in named(node)
                .into_iter()
                .filter(|clause| clause.kind() == FOR_IN)
                .filter_map(|clause| clause.child_by_field_name(RIGHT_FIELD))
            {
                collect_names(iterable, source, names);
            }
        }
        LAMBDA => {
            if let Some(body) = node.child_by_field_name(BODY_FIELD) {
                collect_names(body, source, names);
            }
            let parameters = lambda_parameters(node, source);
            names.retain(|(name, _)| !parameters.contains(name));
        }
        _ => {
            for child in named(node) {
                collect_names(child, source, names);
            }
        }
    }
}

// Los parámetros de una lambda que no son `*args` ni `**kwargs`.
fn lambda_parameters<'source>(lambda: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    lambda
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|parameter| match parameter.kind() {
            IDENTIFIER => Some(parameter),
            TYPED_PARAMETER => named(parameter)
                .into_iter()
                .find(|child| child.kind() == IDENTIFIER),
            kind if DEFAULTED_PARAMETERS.contains(&kind) => parameter.child_by_field_name(NAME_FIELD),
            _ => None,
        })
        .map(|name| text(name, source))
        .collect()
}
