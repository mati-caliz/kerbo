use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use crate::syntax::named_children;

const TYPE_DECLARATIONS: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "record_declaration",
    "interface_declaration",
];
const VARIABLE_OWNERS: &[&str] = &[
    "local_variable_declaration",
    "field_declaration",
    "constant_declaration",
];
const NAMED_VARIABLES: &[&str] = &["formal_parameter", "enhanced_for_statement", "resource"];
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const CREATION: &str = "object_creation_expression";
const CLASS_BODY: &str = "class_body";
const PROGRAM: &str = "program";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const THIS: &str = "this";
const TYPE_FIELD: &str = "type";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const SUPERCLASS: &str = "superclass";
const INTERFACES_FIELD: &str = "interfaces";
const EXTENDED_INTERFACES: &str = "extends_interfaces";
const TYPE_LIST: &str = "type_list";
const INFERRED: &str = "var";
const LAMBDA: &str = "lambda_expression";
const INFERRED_PARAMETERS: &str = "inferred_parameters";
const PARAMETERS_FIELD: &str = "parameters";
const IMPORT: &str = "import_declaration";
const ASTERISK: &str = "asterisk";
const STATIC: &str = "static ";
const GENERIC_OPEN: char = '<';
const QUALIFIER_SEPARATOR: char = '.';
const ARRAY_OPEN: char = '[';

// Dónde empieza a buscarse el método de una llamada: en un tipo conocido (`repo.buscar()` con
// `Repositorio repo`, o `Clase.metodo()`), en las clases que encierran la llamada (`metodo()` o
// `this.metodo()`), o en ninguna parte que se sepa sin compilar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receiver {
    Unknown,
    Type(String),
    Scope(Vec<String>),
}

// Por nombre, el tipo con que se declara cada variable, parámetro o campo de la clase de nivel
// superior; si el mismo nombre se declara con tipos distintos (o con `var`), no se sabe.
pub type Variables = HashMap<String, Option<String>>;

pub fn variable_types(nodes: &[Node<'_>], source: &[u8]) -> Variables {
    let mut variables = Variables::new();
    let mut declare = |variable: Node<'_>, written: Option<Node<'_>>| {
        let declared = written
            .map(|node| simple_type_name(text(node, source)))
            .filter(|declared| declared != INFERRED);
        match variables.entry(text(variable, source).to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert(declared);
            }
            Entry::Occupied(mut entry) if *entry.get() != declared => {
                entry.insert(None);
            }
            Entry::Occupied(_) => {}
        }
    };
    for (variable, written) in nodes.iter().flat_map(|node| declared_by(*node)) {
        declare(variable, written);
    }
    variables
}

// Las variables que declara un nodo, con el tipo escrito si lo tiene.
fn declared_by(node: Node<'_>) -> Vec<(Node<'_>, Option<Node<'_>>)> {
    if VARIABLE_OWNERS.contains(&node.kind()) {
        let written = node.child_by_field_name(TYPE_FIELD);
        return named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
            .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
            .map(|variable| (variable, written))
            .collect();
    }
    if node.kind() == LAMBDA {
        return inferred_parameters(node)
            .into_iter()
            .map(|variable| (variable, None))
            .collect();
    }
    if NAMED_VARIABLES.contains(&node.kind()) {
        return node
            .child_by_field_name(NAME_FIELD)
            .map(|variable| (variable, node.child_by_field_name(TYPE_FIELD)))
            .into_iter()
            .collect();
    }
    Vec::new()
}

// Los parámetros de una lambda sin tipo escrito (`x -> ...`, `(a, b) -> ...`).
fn inferred_parameters(lambda: Node<'_>) -> Vec<Node<'_>> {
    let Some(parameters) = lambda.child_by_field_name(PARAMETERS_FIELD) else {
        return Vec::new();
    };
    match parameters.kind() {
        IDENTIFIER => vec![parameters],
        INFERRED_PARAMETERS => named_children(parameters)
            .into_iter()
            .filter(|child| child.kind() == IDENTIFIER)
            .collect(),
        _ => Vec::new(),
    }
}

fn receiver_of(invocation: Node<'_>, source: &[u8], variables: &Variables) -> Receiver {
    let Some(object) = invocation.child_by_field_name(OBJECT_FIELD) else {
        return Receiver::Scope(enclosing_types(invocation, source));
    };
    match object.kind() {
        THIS => Receiver::Scope(enclosing_types(invocation, source)),
        IDENTIFIER => {
            let name = text(object, source);
            match variables.get(name) {
                Some(Some(declared)) => Receiver::Type(declared.clone()),
                None if name.starts_with(|first: char| first.is_ascii_uppercase()) => {
                    Receiver::Type(name.to_owned())
                }
                Some(None) | None => Receiver::Unknown,
            }
        }
        FIELD_ACCESS
            if object
                .child_by_field_name(OBJECT_FIELD)
                .is_some_and(|owner| owner.kind() == THIS) =>
        {
            object
                .child_by_field_name(FIELD_FIELD)
                .and_then(|field| variables.get(text(field, source)).cloned().flatten())
                .map_or(Receiver::Unknown, Receiver::Type)
        }
        _ => Receiver::Unknown,
    }
}

// Las clases que encierran un nodo, de la más cercana a la más lejana; una clase anónima cuenta
// como el tipo que crea.
fn enclosing_types(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut types = Vec::new();
    let mut current = node.parent();
    while let Some(candidate) = current {
        let kind = candidate.kind();
        if kind == PROGRAM {
            break;
        }
        if TYPE_DECLARATIONS.contains(&kind) {
            types.extend(
                candidate
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source).to_owned()),
            );
        } else if kind == CREATION
            && named_children(candidate)
                .iter()
                .any(|child| child.kind() == CLASS_BODY)
        {
            types.extend(
                candidate
                    .child_by_field_name(TYPE_FIELD)
                    .map(|written| simple_type_name(text(written, source))),
            );
        }
        current = candidate.parent();
    }
    types
}

// Los padres que declara un tipo: la clase que extiende y las interfaces que implementa o
// extiende, por nombre simple.
pub fn supertypes(declaration: Node<'_>, source: &[u8]) -> Vec<String> {
    [SUPERCLASS, INTERFACES_FIELD]
        .iter()
        .filter_map(|field| declaration.child_by_field_name(field))
        .chain(
            named_children(declaration)
                .into_iter()
                .filter(|child| child.kind() == EXTENDED_INTERFACES || child.kind() == SUPERCLASS),
        )
        .flat_map(|clause| {
            named_children(clause).into_iter().flat_map(|child| {
                if child.kind() == TYPE_LIST {
                    named_children(child)
                } else {
                    vec![child]
                }
            })
        })
        .map(|written| simple_type_name(text(written, source)))
        .collect()
}

// Un método importado con `import static` (o con un comodín) se llama sin calificar desde afuera de
// su clase: esas llamadas no se sabe adónde van.
pub struct StaticImports {
    names: Vec<String>,
    any: bool,
}

impl StaticImports {
    pub fn of(top_level: Node<'_>, source: &[u8]) -> Self {
        let mut imports = Self {
            names: Vec::new(),
            any: false,
        };
        let declarations = top_level.parent().map(named_children).unwrap_or_default();
        for import in declarations
            .into_iter()
            .filter(|node| node.kind() == IMPORT && text(*node, source).contains(STATIC))
        {
            let children = named_children(import);
            if children.iter().any(|child| child.kind() == ASTERISK) {
                imports.any = true;
            }
            imports.names.extend(
                children
                    .iter()
                    .filter(|child| child.kind() != ASTERISK)
                    .map(|path| {
                        let written = text(*path, source);
                        written
                            .rsplit(QUALIFIER_SEPARATOR)
                            .next()
                            .unwrap_or(written)
                            .to_owned()
                    }),
            );
        }
        imports
    }

    pub fn receiver(&self, invocation: Node<'_>, source: &[u8], variables: &Variables) -> Receiver {
        let resolved = receiver_of(invocation, source, variables);
        let imported = self.any
            || invocation
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| self.names.iter().any(|known| known == text(name, source)));
        if imported && matches!(resolved, Receiver::Scope(_)) {
            Receiver::Unknown
        } else {
            resolved
        }
    }
}

pub fn simple_type_name(written: &str) -> String {
    let without_arguments = written.split(GENERIC_OPEN).next().unwrap_or(written);
    let without_arrays = without_arguments
        .split(ARRAY_OPEN)
        .next()
        .unwrap_or(without_arguments);
    without_arrays
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(without_arrays)
        .trim()
        .to_owned()
}
