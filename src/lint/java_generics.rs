use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::TYPE_DECLARATIONS;
use super::java_receivers::{simple_type_name, supertype_nodes, type_arguments};
use crate::syntax::{descendants, named_children, one_based};

const METHODS: &[&str] = &["method_declaration", "constructor_declaration"];
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const TYPE_IDENTIFIER: &str = "type_identifier";

// Los parámetros de tipo de las clases y los métodos de un archivo, y los padres de cada clase con
// los argumentos que les pasa: con eso se sabe qué es el `E` de un `throws E` heredado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Generics {
    pub types: Vec<TypeGenerics>,
    pub methods: Vec<MethodGenerics>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeGenerics {
    pub name: String,
    pub parameters: Vec<String>,
    pub parents: Vec<GenericParent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParent {
    pub written: String,
    pub arguments: Vec<String>,
}

// Un método ubicado por la posición de su nombre, como en las declaraciones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodGenerics {
    pub line: usize,
    pub column: usize,
    pub parameters: Vec<String>,
}

pub fn generics_of(root: Node<'_>, source: &[u8]) -> Generics {
    let mut generics = Generics::default();
    for node in descendants(root) {
        let Some(name) = node.child_by_field_name(NAME_FIELD) else {
            continue;
        };
        if TYPE_DECLARATIONS.contains(&node.kind()) {
            generics
                .types
                .extend(type_generics(node, text(name, source), source));
        } else if METHODS.contains(&node.kind()) {
            let parameters = type_parameters(node, source);
            let position = name.start_position();
            generics
                .methods
                .extend((!parameters.is_empty()).then(|| MethodGenerics {
                    line: one_based(position.row),
                    column: one_based(position.column),
                    parameters,
                }));
        }
    }
    generics
}

fn type_generics(declaration: Node<'_>, name: &str, source: &[u8]) -> Option<TypeGenerics> {
    let parameters = type_parameters(declaration, source);
    let parents: Vec<GenericParent> = supertype_nodes(declaration)
        .into_iter()
        .map(|written| GenericParent {
            written: simple_type_name(text(written, source)),
            arguments: type_arguments(written, source),
        })
        .collect();
    let generic = !parameters.is_empty() || parents.iter().any(|parent| !parent.arguments.is_empty());
    generic.then(|| TypeGenerics {
        name: name.to_owned(),
        parameters,
        parents,
    })
}

fn type_parameters(declaration: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == TYPE_PARAMETERS)
        .flat_map(named_children)
        .filter(|parameter| parameter.kind() == TYPE_PARAMETER)
        .filter_map(|parameter| {
            named_children(parameter)
                .into_iter()
                .find(|part| part.kind() == TYPE_IDENTIFIER)
        })
        .map(|identifier| text(identifier, source).to_owned())
        .collect()
}
