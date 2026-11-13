use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::annotated_with;
use super::java_fields::{Volatility, volatility_of};
use crate::syntax::{descendants, named_children};

const NULLABLE: &str = "Nullable";
const CHECK_FOR_NULL: &str = "CheckForNull";
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
const QUALIFIER_SEPARATOR: char = '.';
const ANNOTATION_KINDS: &[&str] = &["marker_annotation", "annotation"];
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

// Si la firma del método declara que el retorno puede ser null.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnNullness {
    Unannotated,
    Nullable,
}

pub fn return_nullness(method: Option<Node<'_>>, source: &[u8]) -> ReturnNullness {
    if method.is_some_and(|declaration| declares_nullable_type(declaration, source)) {
        ReturnNullness::Nullable
    } else {
        ReturnNullness::Unannotated
    }
}

// Las claves con las que el flujo sigue a cada campo `@Nullable` que el método ve: `this.x` siempre
// y `x` si el método no declara nada con ese nombre. Las dos de un mismo campo van juntas, con si
// el campo es `volatile`.
pub(super) fn nullable_field_keys(method: Node<'_>, source: &[u8]) -> Vec<(Vec<String>, Volatility)> {
    let declared = declared_names(method, source);
    nullable_fields(method, source)
        .into_iter()
        .map(|(field, volatility)| {
            let mut keys = vec![format!("{THIS_PREFIX}{field}")];
            if !declared.contains(&field) {
                keys.push(field);
            }
            (keys, volatility)
        })
        .collect()
}

// Los campos `@Nullable` de las clases que encierran al método, de adentro hacia afuera: un campo
// de una clase de más adentro tapa al del mismo nombre de una de afuera.
fn nullable_fields(method: Node<'_>, source: &[u8]) -> Vec<(String, Volatility)> {
    let mut nullable_by_name: HashMap<String, Option<Volatility>> = HashMap::new();
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
        .filter_map(|(name, nullable)| nullable.map(|volatility| (name, volatility)))
        .collect()
}

// Los campos que declara el cuerpo de una clase y, si son `@Nullable`, si son `volatile`.
fn declared_fields(body: Node<'_>, source: &[u8]) -> Vec<(String, Option<Volatility>)> {
    named_children(body)
        .into_iter()
        .filter(|child| child.kind() == FIELD_DECLARATION)
        .flat_map(|declaration| {
            let volatility = volatility_of(declaration, source);
            named_children(declaration)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(move |declarator| {
                    let name = declarator.child_by_field_name(NAME_FIELD)?;
                    Some((
                        text(name, source).to_owned(),
                        declares_nullable(declarator, source).then_some(volatility),
                    ))
                })
        })
        .collect()
}

// Si la variable que declara un campo puede ser null: el campo es `@Nullable` y la variable no
// agrega dimensiones (`@Nullable Object celdas[]` habla de los elementos).
pub fn declares_nullable(declarator: Node<'_>, source: &[u8]) -> bool {
    declarator.child_by_field_name(DIMENSIONS_FIELD).is_none()
        && declarator
            .parent()
            .is_some_and(|field| declares_nullable_type(field, source))
}

// Si el campo o el método declara su tipo `@Nullable` o `@CheckForNull`. Con anotaciones de tipo,
// `@Nullable Object[]` habla de los elementos: el arreglo es nulo sólo con `Object @Nullable []`.
pub fn declares_nullable_type(declaration: Node<'_>, source: &[u8]) -> bool {
    let Some(written) = declaration.child_by_field_name(TYPE_FIELD) else {
        return false;
    };
    if written.kind() == ARRAY_TYPE {
        return written
            .child_by_field_name(DIMENSIONS_FIELD)
            .is_some_and(|dimensions| outer_dimension(text(dimensions, source)).contains(NULLABLE));
    }
    [NULLABLE, CHECK_FOR_NULL].into_iter().any(|annotation| {
        annotated_with(declaration, annotation, source)
            || annotated_after_type_parameters(declaration, annotation, source)
    }) || has_nullable_last_segment(written, source)
}

// En `<T> @Nullable T get()` la anotación va después de los parámetros de tipo, fuera de los
// modificadores.
fn annotated_after_type_parameters(declaration: Node<'_>, simple_name: &str, source: &[u8]) -> bool {
    named_children(declaration)
        .into_iter()
        .filter(|child| ANNOTATION_KINDS.contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .any(|name| text(name, source).rsplit(QUALIFIER_SEPARATOR).next() == Some(simple_name))
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
