use tree_sitter::Node;

use super::classes::{
    CLASS_VAR, DEFINITION_FIELD, MODULE_SEPARATOR, SeenModules, annotation_is, assignments,
    dataclass_decorator,
};
use super::model::{BindingKind, Resolution};
use super::mutability::Resolver;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, BODY_FIELD, CALL, CLASS, FUNCTION, FUNCTION_FIELD,
    IDENTIFIER, KEYWORD_ARGUMENT, NAME_FIELD, OBJECT_FIELD, TRUE, VALUE_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const DATACLASS_FIELDS: [&str; 5] = [
    "dataclasses.field",
    "attrs.field",
    "attr.ib",
    "attr.attrib",
    "attr.field",
];
const DESCRIPTOR_METHODS: [&str; 3] = ["__get__", "__set__", "__delete__"];
const FROZEN: &str = "frozen";

// RUF009: `campo: list = armar()` en una dataclass, una llamada que se evalúa una sola vez y comparten
// todas las instancias. Como ruff, salvo `field(...)`, tipos o funciones inmutables, `ClassVar`,
// descriptores, `NewType` de algo inmutable y dataclasses congeladas.
pub fn dataclass_call_defaults(
    resolver: &Resolver<'_, '_>,
    seen: &SeenModules,
    class: Node<'_>,
) -> Vec<Report> {
    assignments(class)
        .into_iter()
        .filter_map(|assignment| {
            let annotation = assignment.annotation?;
            let call = unparenthesized(assignment.value);
            let function = call
                .child_by_field_name(FUNCTION_FIELD)
                .filter(|_| call.kind() == CALL)?;
            let exempt = resolver
                .qualified(function)
                .is_some_and(|name| DATACLASS_FIELDS.contains(&name.as_str()))
                || resolver.is_immutable_annotation(annotation)
                || (seen.typing && annotation_is(resolver, annotation, CLASS_VAR))
                || resolver.is_immutable_function(function)
                || resolver.is_immutable_new_type(function)
                || local_class(resolver, function).is_some_and(|local| {
                    is_descriptor(local, resolver.source) || is_frozen_dataclass(resolver, local)
                });
            let name = dotted_name(function, resolver.source).unwrap_or_default();
            (!exempt).then(|| Report::at(PythonRule::RUF009, call, &name))
        })
        .collect()
}

fn dotted_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        IDENTIFIER => Some(text(node, source).to_owned()),
        ATTRIBUTE => {
            let object = dotted_name(node.child_by_field_name(OBJECT_FIELD)?, source)?;
            let attribute = text(node.child_by_field_name(ATTRIBUTE_FIELD)?, source);
            Some(format!("{object}{MODULE_SEPARATOR}{attribute}"))
        }
        _ => None,
    }
}

// La clase del archivo a la que apunta un nombre, desde donde está la dataclass.
fn local_class<'tree>(resolver: &Resolver<'_, 'tree>, function: Node<'_>) -> Option<Node<'tree>> {
    if function.kind() != IDENTIFIER {
        return None;
    }
    let Resolution::Binding(id) = resolver
        .model
        .lookup_from(resolver.scope, text(function, resolver.source))
    else {
        return None;
    };
    resolver
        .model
        .binding(id)
        .filter(|binding| binding.kind == BindingKind::Class)
        .and_then(|binding| binding.site.statement)
        .map(|statement| {
            statement
                .child_by_field_name(DEFINITION_FIELD)
                .unwrap_or(statement)
        })
        .filter(|statement| statement.kind() == CLASS)
}

fn is_descriptor(class: Node<'_>, source: &[u8]) -> bool {
    class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .map(|statement| {
            statement
                .child_by_field_name(DEFINITION_FIELD)
                .unwrap_or(statement)
        })
        .filter(|statement| statement.kind() == FUNCTION)
        .filter_map(|function| function.child_by_field_name(NAME_FIELD))
        .any(|name| DESCRIPTOR_METHODS.contains(&text(name, source)))
}

fn is_frozen_dataclass(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    dataclass_decorator(resolver, class)
        .filter(|decorator| decorator.kind() == CALL)
        .and_then(|call| call.child_by_field_name(ARGUMENTS_FIELD))
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .any(|keyword| {
            keyword
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, resolver.source) == FROZEN)
                && keyword
                    .child_by_field_name(VALUE_FIELD)
                    .is_some_and(|value| value.kind() == TRUE)
        })
}
