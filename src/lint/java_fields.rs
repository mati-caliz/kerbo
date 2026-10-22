use std::collections::BTreeSet;

use tree_sitter::Node;

use super::java::{NAME_FIELD, STATIC, text};
use super::java_calls::{Visibility, has_modifier, visibility};
use super::java_owner::{Owner, annotation_names, owner_of};
use super::java_receivers::simple_type_name;
use crate::syntax::named_children;

const FIELD: &str = "field_declaration";
const INTERFACE_FIELD: &str = "constant_declaration";
const FIELD_DECLARATIONS: &[&str] = &[FIELD, INTERFACE_FIELD];
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const IDENTIFIER: &str = "identifier";
const TYPE_FIELD: &str = "type";

// Un campo declarado, con lo que hace falta para saber si un framework lo usa.
pub struct DeclaredField<'tree> {
    pub name: String,
    pub name_node: Node<'tree>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub type_name: String,
    pub annotations: Vec<String>,
    pub owner: Owner,
}

pub fn declared_fields<'tree>(nodes: &[Node<'tree>], source: &[u8]) -> Vec<DeclaredField<'tree>> {
    nodes
        .iter()
        .filter(|node| FIELD_DECLARATIONS.contains(&node.kind()))
        .flat_map(|&declaration| {
            let visibility = visibility(declaration, source);
            let is_static =
                declaration.kind() == INTERFACE_FIELD || has_modifier(declaration, STATIC, source);
            let annotations = annotation_names(declaration, source);
            let type_name = declaration
                .child_by_field_name(TYPE_FIELD)
                .map(|written| simple_type_name(text(written, source)))
                .unwrap_or_default();
            let owner = owner_of(declaration, source);
            field_names(declaration)
                .into_iter()
                .map(move |name_node| DeclaredField {
                    name: text(name_node, source).to_owned(),
                    name_node,
                    visibility,
                    is_static,
                    type_name: type_name.clone(),
                    annotations: annotations.clone(),
                    owner: owner.clone(),
                })
        })
        .collect()
}

// Los nombres que usa el código, sin los de las declaraciones de campos: una lectura, una
// escritura, `this.nombre`, un `import static` o una etiqueta de `case` cuentan igual.
pub fn used_identifiers(nodes: &[Node<'_>], source: &[u8]) -> Vec<String> {
    let declared: BTreeSet<usize> = nodes
        .iter()
        .filter(|node| FIELD_DECLARATIONS.contains(&node.kind()))
        .flat_map(|declaration| field_names(*declaration))
        .map(|name| name.id())
        .collect();
    nodes
        .iter()
        .filter(|node| node.kind() == IDENTIFIER && !declared.contains(&node.id()))
        .map(|node| text(*node, source).to_owned())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

fn field_names(declaration: Node<'_>) -> Vec<Node<'_>> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .collect()
}
