use std::collections::HashMap;

use tree_sitter::Node;

use super::imports::Imports;
use crate::syntax::{children_with_fields, descendants, named_children};

const PARAMETER: &str = "parameter_declaration";
const VARIADIC_PARAMETER: &str = "variadic_parameter_declaration";
const VAR_SPEC: &str = "var_spec";
const VAR_DECLARATION: &str = "var_declaration";
const TYPE_SPEC: &str = "type_spec";
const STRUCT_TYPE: &str = "struct_type";
const FIELD_DECLARATION: &str = "field_declaration";
const FIELD_SEPARATOR: char = '.';
const POINTER: &str = "pointer_type";
const QUALIFIED: &str = "qualified_type";
const TYPE_IDENTIFIER: &str = "type_identifier";
const GENERIC: &str = "generic_type";
const PARENTHESIZED: &str = "parenthesized_type";
const NAME_FIELD: &str = "name";
const TYPE_FIELD: &str = "type";
// Un tipo del propio paquete: empieza con `@` para que el índice lo complete con la carpeta.
pub const LOCAL_PREFIX: &str = "@";

// Los tipos declarados de las variables que se ven desde una función: los parámetros, los `var x T`
// de ella y de las funciones que la contienen, y las variables del paquete. También los campos de
// los structs del archivo, como `@Handler.db`.
#[derive(Debug, Default)]
pub struct Types {
    declared: HashMap<String, String>,
    fields: HashMap<String, String>,
}

impl Types {
    pub fn of(scope: Node<'_>, root: Node<'_>, source: &[u8], imports: &Imports) -> Self {
        let package_level = named_children(root)
            .into_iter()
            .filter(|node| node.kind() == VAR_DECLARATION)
            .flat_map(descendants);
        let declared = package_level
            .chain(descendants(scope))
            .filter(|node| matches!(node.kind(), PARAMETER | VARIADIC_PARAMETER | VAR_SPEC))
            .flat_map(|declaration| typed_names(declaration, source, imports))
            .collect();
        let fields = descendants(root)
            .into_iter()
            .filter(|node| node.kind() == TYPE_SPEC)
            .flat_map(|spec| struct_fields(spec, source, imports))
            .collect();
        Self { declared, fields }
    }

    pub fn field(&self, owner: &str, field: &str) -> Option<&str> {
        self.fields
            .get(&format!("{owner}{FIELD_SEPARATOR}{field}"))
            .map(String::as_str)
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.declared.get(name).map(String::as_str)
    }

    pub fn declare(&mut self, name: &str, written: String) {
        self.declared.insert(name.to_owned(), written);
    }
}

fn struct_fields(spec: Node<'_>, source: &[u8], imports: &Imports) -> Vec<(String, String)> {
    let (Some(name), Some(body)) = (
        spec.child_by_field_name(NAME_FIELD),
        spec.child_by_field_name(TYPE_FIELD),
    ) else {
        return Vec::new();
    };
    if body.kind() != STRUCT_TYPE {
        return Vec::new();
    }
    let owner = format!("{LOCAL_PREFIX}{}", name.utf8_text(source).unwrap_or_default());
    descendants(body)
        .into_iter()
        .filter(|node| node.kind() == FIELD_DECLARATION)
        .flat_map(|declaration| typed_names(declaration, source, imports))
        .map(|(field, written)| (format!("{owner}{FIELD_SEPARATOR}{field}"), written))
        .collect()
}

fn typed_names(declaration: Node<'_>, source: &[u8], imports: &Imports) -> Vec<(String, String)> {
    let Some(written) = declaration
        .child_by_field_name(TYPE_FIELD)
        .and_then(|typ| type_name(typ, source, imports))
    else {
        return Vec::new();
    };
    children_with_fields(declaration)
        .into_iter()
        .filter(|(field, _)| *field == Some(NAME_FIELD))
        .map(|(_, name)| {
            (
                name.utf8_text(source).unwrap_or_default().to_owned(),
                written.clone(),
            )
        })
        .collect()
}

// `*sql.DB` es `database/sql.DB`, `Store` es `@Store`; los slices, mapas y funciones no cuentan.
pub fn type_name(node: Node<'_>, source: &[u8], imports: &Imports) -> Option<String> {
    let text = node.utf8_text(source).unwrap_or_default();
    match node.kind() {
        POINTER | PARENTHESIZED => named_children(node)
            .first()
            .and_then(|inner| type_name(*inner, source, imports)),
        GENERIC => node
            .child_by_field_name(TYPE_FIELD)
            .and_then(|inner| type_name(inner, source, imports)),
        QUALIFIED => Some(imports.qualify(text)),
        TYPE_IDENTIFIER => Some(format!("{LOCAL_PREFIX}{text}")),
        _ => None,
    }
}

// Los parámetros en orden, uno por nombre (`a, b string` son dos), con su tipo.
pub fn parameters<'tree>(list: Node<'tree>) -> Vec<(Option<Node<'tree>>, Option<Node<'tree>>)> {
    named_children(list)
        .into_iter()
        .filter(|node| matches!(node.kind(), PARAMETER | VARIADIC_PARAMETER))
        .flat_map(|declaration| {
            let typ = declaration.child_by_field_name(TYPE_FIELD);
            let names: Vec<Node<'tree>> = children_with_fields(declaration)
                .into_iter()
                .filter(|(field, _)| *field == Some(NAME_FIELD))
                .map(|(_, name)| name)
                .collect();
            if names.is_empty() {
                vec![(None, typ)]
            } else {
                names.into_iter().map(|name| (Some(name), typ)).collect()
            }
        })
        .collect()
}
