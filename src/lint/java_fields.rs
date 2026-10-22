use std::collections::{BTreeSet, HashMap};

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
const FIELD_ACCESS: &str = "field_access";
const THIS: &str = "this";
const ASSIGNMENT: &str = "assignment_expression";
const FIELD_FIELD: &str = "field";
const OBJECT_FIELD: &str = "object";
const LEFT_FIELD: &str = "left";
const OPERATOR_FIELD: &str = "operator";
const SIMPLE_ASSIGNMENT: &str = "=";
const SCOPES: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "lambda_expression",
];
const CLASS_BODY: &str = "class_body";
const INFERRED_PARAMETERS: &str = "inferred_parameters";
const PARAMETERS_FIELD: &str = "parameters";
const LOCAL_VARIABLES: &str = "local_variable_declaration";
const SPREAD_PARAMETER: &str = "spread_parameter";
const LOCAL_DECLARERS: &[&str] = &[
    "formal_parameter",
    "catch_formal_parameter",
    "enhanced_for_statement",
    "resource",
];

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
    let declared = declared_field_names(nodes);
    nodes
        .iter()
        .filter(|node| node.kind() == IDENTIFIER && !declared.contains(&node.id()))
        .map(|node| text(*node, source).to_owned())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

// Los nombres que el código sólo escribe (`this.total = x` o `total = x`) y nunca lee: un `+=` o un
// `++` también leen. Adentro de un método, un nombre que es un parámetro o una variable local no
// lee el campo que se llama igual (`this.total = total` en un constructor).
pub fn written_only_identifiers(nodes: &[Node<'_>], source: &[u8]) -> Vec<String> {
    let declared = declared_field_names(nodes);
    let mut locals: HashMap<usize, BTreeSet<&str>> = HashMap::new();
    let mut written = BTreeSet::new();
    let mut read = BTreeSet::new();
    for node in nodes
        .iter()
        .filter(|node| node.kind() == IDENTIFIER && !declared.contains(&node.id()))
    {
        let name = text(*node, source);
        if is_assigned(*node, source) {
            written.insert(name);
        } else if is_field_access(*node) || !is_local(*node, name, source, &mut locals) {
            read.insert(name);
        }
    }
    written.difference(&read).map(|name| (*name).to_owned()).collect()
}

fn declared_field_names(nodes: &[Node<'_>]) -> BTreeSet<usize> {
    nodes
        .iter()
        .filter(|node| FIELD_DECLARATIONS.contains(&node.kind()))
        .flat_map(|declaration| field_names(*declaration))
        .map(|name| name.id())
        .collect()
}

// Si el nombre es un parámetro o una variable local del método o la lambda más cercana; una clase
// anidada corta la búsqueda (adentro, el nombre puede ser un campo suyo).
fn is_local<'source>(
    identifier: Node<'_>,
    name: &str,
    source: &'source [u8],
    locals: &mut HashMap<usize, BTreeSet<&'source str>>,
) -> bool {
    let mut current = identifier.parent();
    while let Some(node) = current {
        if node.kind() == CLASS_BODY {
            return false;
        }
        if SCOPES.contains(&node.kind()) {
            return locals
                .entry(node.id())
                .or_insert_with(|| local_names(node, source))
                .contains(name);
        }
        current = node.parent();
    }
    false
}

// Los parámetros y las variables que declara un método o una lambda, sin entrar a las clases ni a las
// lambdas que tenga adentro.
fn local_names<'source>(scope: Node<'_>, source: &'source [u8]) -> BTreeSet<&'source str> {
    let mut names = BTreeSet::new();
    let mut pending = vec![scope];
    while let Some(node) = pending.pop() {
        let lambda_parameter = scope
            .child_by_field_name(PARAMETERS_FIELD)
            .is_some_and(|parameters| parameters.id() == node.id())
            || node
                .parent()
                .is_some_and(|parent| parent.kind() == INFERRED_PARAMETERS);
        if node.kind() == IDENTIFIER && lambda_parameter {
            names.insert(text(node, source));
        }
        if LOCAL_DECLARERS.contains(&node.kind())
            && let Some(declared) = node.child_by_field_name(NAME_FIELD)
        {
            names.insert(text(declared, source));
        }
        if node.kind() == LOCAL_VARIABLES || node.kind() == SPREAD_PARAMETER {
            names.extend(
                named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                    .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
                    .map(|declared| text(declared, source)),
            );
        }
        pending.extend(
            named_children(node)
                .into_iter()
                .filter(|child| child.kind() != CLASS_BODY && !SCOPES.contains(&child.kind())),
        );
    }
    names
}

fn is_field_access(identifier: Node<'_>) -> bool {
    identifier
        .parent()
        .is_some_and(|access| access.kind() == FIELD_ACCESS)
}

// El destino de una asignación simple: el nombre solo, o el campo de `this.nombre`.
fn is_assigned(identifier: Node<'_>, source: &[u8]) -> bool {
    let target = match identifier.parent() {
        Some(access)
            if access.kind() == FIELD_ACCESS
                && access
                    .child_by_field_name(FIELD_FIELD)
                    .is_some_and(|field| field.id() == identifier.id())
                && access
                    .child_by_field_name(OBJECT_FIELD)
                    .is_some_and(|object| object.kind() == THIS) =>
        {
            access
        }
        _ => identifier,
    };
    target.parent().is_some_and(|assignment| {
        assignment.kind() == ASSIGNMENT
            && assignment
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.id() == target.id())
            && assignment
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == SIMPLE_ASSIGNMENT)
    })
}

fn field_names(declaration: Node<'_>) -> Vec<Node<'_>> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .collect()
}
