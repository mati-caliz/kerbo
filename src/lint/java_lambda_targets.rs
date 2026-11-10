use std::collections::BTreeSet;

use tree_sitter::Node;

use super::java::{
    CONSTRUCTOR, FORMAL_PARAMETER, IDENTIFIER, METHOD, NAME_FIELD, OBJECT_CREATION, PARAMETERS_FIELD,
    SPREAD_PARAMETER, TYPE_FIELD, TYPE_IDENTIFIER, text,
};
use super::java_receivers::simple_type_name;
use super::java_throws::{LAMBDA, METHOD_REFERENCE};
use crate::syntax::{descendants, named_children};

const ARGUMENT_LIST: &str = "argument_list";
const INVOCATION: &str = "method_invocation";
const FORMAL_PARAMETERS: &str = "formal_parameters";
const INFERRED_PARAMETERS: &str = "inferred_parameters";
const SINGLE_PARAMETER: usize = 1;

// Lo que hace falta para adivinar a qué interfaz puede apuntar una lambda sin compilar: cada lambda
// (con su cantidad de parámetros; una referencia a método no la dice) y la llamada de la que es
// argumento, los tipos que nombra el archivo y el tipo de cada parámetro de sus métodos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LambdaTargets {
    pub sites: Vec<LambdaSite>,
    pub named_types: Vec<String>,
    pub parameter_types: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LambdaSite {
    pub arity: Option<usize>,
    pub argument_of: Option<String>,
}

pub fn lambda_targets_of(root: Node<'_>, source: &[u8]) -> LambdaTargets {
    let mut sites = BTreeSet::new();
    let mut named_types = BTreeSet::new();
    let mut parameter_types = BTreeSet::new();
    for node in descendants(root) {
        match node.kind() {
            TYPE_IDENTIFIER => {
                named_types.insert(text(node, source).to_owned());
            }
            LAMBDA => {
                sites.insert(LambdaSite {
                    arity: Some(lambda_arity(node)),
                    argument_of: argument_of(node, source),
                });
            }
            METHOD_REFERENCE => {
                sites.insert(LambdaSite {
                    arity: None,
                    argument_of: argument_of(node, source),
                });
            }
            METHOD | CONSTRUCTOR => parameter_types.extend(typed_parameters(node, source)),
            _ => {}
        }
    }
    LambdaTargets {
        sites: sites.into_iter().collect(),
        named_types: named_types.into_iter().collect(),
        parameter_types: parameter_types.into_iter().collect(),
    }
}

fn lambda_arity(lambda: Node<'_>) -> usize {
    let Some(parameters) = lambda.child_by_field_name(PARAMETERS_FIELD) else {
        return SINGLE_PARAMETER;
    };
    let counted: &[&str] = match parameters.kind() {
        FORMAL_PARAMETERS => &[FORMAL_PARAMETER, SPREAD_PARAMETER],
        INFERRED_PARAMETERS => &[IDENTIFIER],
        _ => return SINGLE_PARAMETER,
    };
    named_children(parameters)
        .into_iter()
        .filter(|parameter| counted.contains(&parameter.kind()))
        .count()
}

fn argument_of(node: Node<'_>, source: &[u8]) -> Option<String> {
    let arguments = node.parent().filter(|parent| parent.kind() == ARGUMENT_LIST)?;
    let call = arguments.parent()?;
    match call.kind() {
        INVOCATION => call
            .child_by_field_name(NAME_FIELD)
            .map(|name| text(name, source).to_owned()),
        OBJECT_CREATION => call
            .child_by_field_name(TYPE_FIELD)
            .map(|written| simple_type_name(text(written, source))),
        _ => None,
    }
}

fn typed_parameters(callable: Node<'_>, source: &[u8]) -> Vec<(String, String)> {
    let Some(name) = callable.child_by_field_name(NAME_FIELD) else {
        return Vec::new();
    };
    callable
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| [FORMAL_PARAMETER, SPREAD_PARAMETER].contains(&parameter.kind()))
        .filter_map(|parameter| parameter.child_by_field_name(TYPE_FIELD))
        .map(|written| {
            (
                text(name, source).to_owned(),
                simple_type_name(text(written, source)),
            )
        })
        .collect()
}
