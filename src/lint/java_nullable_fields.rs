use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::annotated_with;
use crate::syntax::{descendants, named_children};

const NULLABLE: &str = "Nullable";
const THIS_PREFIX: &str = "this.";
const CLASS_BODIES: &[&str] = &["class_body", "enum_body_declarations"];
const FIELD_DECLARATION: &str = "field_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const TYPE_FIELD: &str = "type";
const DIMENSIONS_FIELD: &str = "dimensions";
const ARRAY_TYPE: &str = "array_type";
const SCOPED_TYPE_IDENTIFIER: &str = "scoped_type_identifier";
const GENERIC_TYPE: &str = "generic_type";
const LAMBDA: &str = "lambda_expression";
const PARAMETERS_FIELD: &str = "parameters";
const IDENTIFIER: &str = "identifier";
const INFERRED_PARAMETERS: &str = "inferred_parameters";
const OPEN_BRACKET: char = '[';
const ANONYMOUS_CLASS_OWNER: &str = "object_creation_expression";
const DECLARING: &[&str] = &[
    "variable_declarator",
    "formal_parameter",
    "catch_formal_parameter",
    "enhanced_for_statement",
    "resource",
    "instanceof_expression",
    "type_pattern",
    "record_pattern_component",
];

// Las claves con las que el flujo sigue a cada campo `@Nullable` que el método ve: `this.x` siempre
// y `x` si el método no declara nada con ese nombre. Las dos de un mismo campo van juntas.
pub(super) fn nullable_field_keys(method: Node<'_>, source: &[u8]) -> Vec<Vec<String>> {
    let declared = declared_names(method, source);
    nullable_fields(method, source)
        .into_iter()
        .map(|field| {
            let mut keys = vec![format!("{THIS_PREFIX}{field}")];
            if !declared.contains(&field) {
                keys.push(field);
            }
            keys
        })
        .collect()
}

// Los campos `@Nullable` de las clases que encierran al método, de adentro hacia afuera: un campo
// de una clase de más adentro tapa al del mismo nombre de una de afuera.
fn nullable_fields(method: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut nullable_by_name: HashMap<String, bool> = HashMap::new();
    let mut current = method.parent();
    while let Some(node) = current {
        if CLASS_BODIES.contains(&node.kind()) {
            for (name, nullable) in declared_fields(node, source) {
                nullable_by_name.entry(name).or_insert(nullable);
            }
            if node
                .parent()
                .is_some_and(|owner| owner.kind() == ANONYMOUS_CLASS_OWNER)
            {
                break;
            }
        }
        current = node.parent();
    }
    nullable_by_name
        .into_iter()
        .filter_map(|(name, nullable)| nullable.then_some(name))
        .collect()
}

// Los campos que declara el cuerpo de una clase y si cada uno es `@Nullable`.
fn declared_fields(body: Node<'_>, source: &[u8]) -> Vec<(String, bool)> {
    named_children(body)
        .into_iter()
        .filter(|child| child.kind() == FIELD_DECLARATION)
        .flat_map(|field| {
            let nullable = is_nullable_field(field, source);
            named_children(field)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(move |declarator| {
                    let name = declarator.child_by_field_name(NAME_FIELD)?;
                    let whole_array = declarator.child_by_field_name(DIMENSIONS_FIELD).is_none();
                    Some((text(name, source).to_owned(), nullable && whole_array))
                })
        })
        .collect()
}

// Con anotaciones de tipo, `@Nullable Object[]` habla de los elementos: el arreglo es nulo sólo
// con `Object @Nullable []`.
fn is_nullable_field(field: Node<'_>, source: &[u8]) -> bool {
    let Some(written) = field.child_by_field_name(TYPE_FIELD) else {
        return false;
    };
    if written.kind() == ARRAY_TYPE {
        return written
            .child_by_field_name(DIMENSIONS_FIELD)
            .is_some_and(|dimensions| outer_dimension(text(dimensions, source)).contains(NULLABLE));
    }
    annotated_with(field, NULLABLE, source) || has_nullable_last_segment(written, source)
}

// En `char[] @Nullable []` la anotación habla de los elementos: el arreglo es el de antes del primer `[`.
fn outer_dimension(dimensions: &str) -> &str {
    dimensions.split(OPEN_BRACKET).next().unwrap_or_default()
}

fn has_nullable_last_segment(written: Node<'_>, source: &[u8]) -> bool {
    let qualified = if written.kind() == GENERIC_TYPE {
        named_children(written).into_iter().next()
    } else {
        Some(written)
    };
    qualified.is_some_and(|scoped| {
        scoped.kind() == SCOPED_TYPE_IDENTIFIER && text(scoped, source).contains(NULLABLE)
    })
}

// Todo nombre que el método declara: parámetros, locales, variables de for, catch, recursos,
// patrones y parámetros de lambdas.
fn declared_names(method: Node<'_>, source: &[u8]) -> HashSet<String> {
    let mut declared = HashSet::new();
    for node in descendants(method) {
        if DECLARING.contains(&node.kind())
            && let Some(name) = node
                .child_by_field_name(NAME_FIELD)
                .filter(|name| name.kind() == IDENTIFIER)
        {
            declared.insert(text(name, source).to_owned());
        }
        if node.kind() == LAMBDA
            && let Some(parameters) = node.child_by_field_name(PARAMETERS_FIELD)
        {
            if parameters.kind() == IDENTIFIER {
                declared.insert(text(parameters, source).to_owned());
            } else if parameters.kind() == INFERRED_PARAMETERS {
                declared.extend(
                    named_children(parameters)
                        .into_iter()
                        .filter(|parameter| parameter.kind() == IDENTIFIER)
                        .map(|parameter| text(parameter, source).to_owned()),
                );
            }
        }
    }
    declared
}
