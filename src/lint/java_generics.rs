use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::TYPE_DECLARATIONS;
use super::java_receivers::{simple_type_name, supertype_nodes, type_arguments};
use crate::syntax::{descendants, named_children, one_based};

const METHODS: &[&str] = &["method_declaration", "constructor_declaration"];
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const TYPE_IDENTIFIER: &str = "type_identifier";
const PARAMETERS_FIELD: &str = "parameters";
const TYPE_FIELD: &str = "type";
const FORMAL_PARAMETER: &str = "formal_parameter";
const FORMAL_PARAMETERS: &[&str] = &["formal_parameter", "spread_parameter"];
const GENERIC_TYPE: &str = "generic_type";

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
    pub carriers: Vec<Carrier>,
}

// Un parámetro del método cuyo tipo lleva un parámetro de tipo del método como argumento:
// `FailableConsumer<T, E> consumer` en la posición 0 lleva el `E` como segundo argumento. Si la
// interfaz tira ese argumento, la lambda que se pasa ahí dice qué es el `E` del `throws E`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carrier {
    pub parameter: String,
    pub position: usize,
    pub interface: String,
    pub argument: usize,
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
                    carriers: carriers_of(node, &parameters, source),
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

// Un parámetro de tipo que aparece en los parámetros de otra forma (`Class<E>`, `E[]`, `List<? extends
// E>`) también decide qué es el `E`: con eso no se puede, y se descartan sus portadores.
fn carriers_of(method: Node<'_>, parameters: &[String], source: &[u8]) -> Vec<Carrier> {
    let Some(formal) = method.child_by_field_name(PARAMETERS_FIELD) else {
        return Vec::new();
    };
    let declared: Vec<Node<'_>> = named_children(formal)
        .into_iter()
        .filter(|parameter| FORMAL_PARAMETERS.contains(&parameter.kind()))
        .collect();
    let direct = direct_carriers(&declared, parameters, source);
    direct
        .iter()
        .filter(|carrier| {
            let carried = direct
                .iter()
                .filter(|other| other.parameter == carrier.parameter)
                .count();
            mentions(&declared, &carrier.parameter, source) == carried
        })
        .cloned()
        .collect()
}

fn direct_carriers(declared: &[Node<'_>], parameters: &[String], source: &[u8]) -> Vec<Carrier> {
    declared
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.kind() == FORMAL_PARAMETER)
        .filter_map(|(position, parameter)| Some((position, parameter.child_by_field_name(TYPE_FIELD)?)))
        .filter(|(_, written)| written.kind() == GENERIC_TYPE)
        .flat_map(|(position, written)| {
            let interface = simple_type_name(text(written, source));
            type_arguments(written, source)
                .into_iter()
                .enumerate()
                .filter(|(_, argument)| parameters.contains(argument))
                .map(move |(argument, parameter)| Carrier {
                    parameter,
                    position,
                    interface: interface.clone(),
                    argument,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn mentions(declared: &[Node<'_>], parameter: &str, source: &[u8]) -> usize {
    declared
        .iter()
        .flat_map(|node| descendants(*node))
        .filter(|node| node.kind() == TYPE_IDENTIFIER && text(*node, source) == parameter)
        .count()
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
