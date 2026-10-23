use std::collections::HashSet;

use tree_sitter::Node;

use crate::lint::python::ast::{
    ALIAS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, AUGMENTED_ASSIGNMENT, CLASS, FUNCTION, IDENTIFIER,
    KEYWORD_ARGUMENT, LEFT_FIELD, NAME_FIELD, named, text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRole {
    Load,
    Store,
    Other,
}

const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_clause";
const NAMED_EXPRESSION: &str = "named_expression";
const AS_PATTERN_TARGET: &str = "as_pattern_target";
const WITH_ITEM: &str = "with_item";
const AS_PATTERN: &str = "as_pattern";
const DELETE: &str = "delete_statement";
const PATTERN_WRAPPERS: [&str; 10] = [
    "tuple",
    "list",
    "parenthesized_expression",
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "expression_list",
    "list_splat",
    "list_splat_pattern",
    "parenthesized_list_splat",
];
// Lugares donde un identificador no es una expresión en el AST de ruff.
const NOT_EXPRESSIONS: [&str; 12] = [
    "parameters",
    "lambda_parameters",
    "typed_parameter",
    "dictionary_splat_pattern",
    "global_statement",
    "nonlocal_statement",
    "dotted_name",
    "aliased_import",
    "case_pattern",
    "keyword_pattern",
    "class_pattern",
    "type_parameter",
];
const DEFAULTED_PARAMETERS: [&str; 2] = ["default_parameter", "typed_default_parameter"];
const SPLAT_PARAMETERS: [&str; 2] = ["list_splat_pattern", "dictionary_splat_pattern"];
const PARAMETER_CONTAINERS: [&str; 2] = ["parameters", "lambda_parameters"];

// Si un identificador se lee, se asigna o no es un nombre (parámetro, atributo, import, etc.).
pub fn name_role(identifier: Node<'_>) -> NameRole {
    let Some(parent) = identifier.parent() else {
        return NameRole::Other;
    };
    if is_field(parent, ATTRIBUTE_FIELD, identifier) && parent.kind() == ATTRIBUTE
        || is_field(parent, NAME_FIELD, identifier)
            && matches!(parent.kind(), KEYWORD_ARGUMENT | FUNCTION | CLASS)
        || is_field(parent, NAME_FIELD, identifier) && DEFAULTED_PARAMETERS.contains(&parent.kind())
        || is_field(parent, ALIAS_FIELD, identifier) && parent.kind() != AS_PATTERN
        || NOT_EXPRESSIONS.contains(&parent.kind())
        || SPLAT_PARAMETERS.contains(&parent.kind()) && is_parameter_splat(parent)
    {
        return NameRole::Other;
    }
    if parent.kind() == NAMED_EXPRESSION && is_field(parent, NAME_FIELD, identifier) {
        return NameRole::Store;
    }
    target_role(identifier)
}

fn is_field(parent: Node<'_>, field: &str, child: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|value| value.id() == child.id())
}

fn is_parameter_splat(splat: Node<'_>) -> bool {
    splat
        .parent()
        .is_some_and(|holder| PARAMETER_CONTAINERS.contains(&holder.kind()))
}

// Sube por tuplas y listas de destinos hasta ver si el identificador está a la izquierda de una
// asignación, de un `for` o de un `with ... as`.
fn target_role(identifier: Node<'_>) -> NameRole {
    let mut child = identifier;
    let mut current = identifier.parent();
    while let Some(parent) = current {
        match parent.kind() {
            kind if PATTERN_WRAPPERS.contains(&kind) => {}
            ASSIGNMENT | AUGMENTED_ASSIGNMENT | FOR | FOR_IN => {
                return if is_field(parent, LEFT_FIELD, child) {
                    NameRole::Store
                } else {
                    NameRole::Load
                };
            }
            AS_PATTERN_TARGET => {
                let in_with = parent
                    .parent()
                    .and_then(|pattern| pattern.parent())
                    .is_some_and(|holder| holder.kind() == WITH_ITEM);
                return if in_with { NameRole::Store } else { NameRole::Other };
            }
            DELETE => return NameRole::Other,
            _ => return NameRole::Load,
        }
        child = parent;
        current = parent.parent();
    }
    NameRole::Load
}

// `Parameters::includes`: todos los nombres de parámetros, también `*args` y `**kwargs`.
pub fn parameter_names<'source>(
    parameters: Option<Node<'_>>,
    source: &'source [u8],
) -> HashSet<&'source str> {
    let mut names = HashSet::new();
    for parameter in parameters.map(named).unwrap_or_default() {
        let name = match parameter.kind() {
            IDENTIFIER => Some(parameter),
            kind if DEFAULTED_PARAMETERS.contains(&kind) => parameter.child_by_field_name(NAME_FIELD),
            _ => named(parameter)
                .into_iter()
                .find(|child| child.kind() == IDENTIFIER),
        };
        names.extend(name.map(|name| text(name, source)));
    }
    names
}
