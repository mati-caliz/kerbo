use std::collections::HashSet;

use tree_sitter::Node;

use super::classes::{
    CLASS_VAR, DEFINITION_FIELD, SeenModules, annotation_is, any_base, assignments, dataclass_decorator,
};
use super::mutability::{Resolver, without_subscript};
use super::parameters::{Signature, has_default};
use super::post_init_fix::{PostInit, initvar_fix};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BODY_FIELD, CALL, DECORATED, FUNCTION, FUNCTION_FIELD, NAME_FIELD, VALUE_FIELD, named, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const STDLIB_DATACLASS: &str = "dataclasses.dataclass";
const POST_INIT: &str = "__post_init__";
const ENUMS: [&str; 7] = [
    "enum.Enum",
    "enum.IntEnum",
    "enum.StrEnum",
    "enum.Flag",
    "enum.IntFlag",
    "enum.ReprEnum",
    "enum.EnumCheck",
];

// Reglas de una dataclass: RUF008 (un campo anotado con un valor mutable, que comparten todas las
// instancias), RUF033 (`__post_init__` con valores por defecto, que nunca se usan) y RUF049 (una
// dataclass que es un `enum`). Las dos últimas, sólo con el `dataclass` de la biblioteca estándar.
pub fn dataclass_reports(resolver: &Resolver<'_, '_>, seen: &SeenModules, class: Node<'_>) -> Vec<Report> {
    let Some(decorator) = dataclass_decorator(resolver, class) else {
        return Vec::new();
    };
    let mut reports = mutable_defaults(resolver, seen, class);
    let callable = if decorator.kind() == CALL {
        decorator.child_by_field_name(FUNCTION_FIELD).unwrap_or(decorator)
    } else {
        decorator
    };
    if resolver.qualified(callable).as_deref() != Some(STDLIB_DATACLASS) {
        return reports;
    }
    if is_enumeration(resolver, class) {
        reports.extend(
            decorator
                .parent()
                .map(|node| Report::at(PythonRule::RUF049, node, "")),
        );
    }
    reports.extend(post_init_defaults(resolver, class));
    reports
}

fn mutable_defaults(resolver: &Resolver<'_, '_>, seen: &SeenModules, class: Node<'_>) -> Vec<Report> {
    assignments(class)
        .into_iter()
        .filter_map(|assignment| {
            let annotation = assignment.annotation?;
            let value = unparenthesized(assignment.value);
            let mutable = resolver.is_mutable_expression(value)
                && !(seen.typing && annotation_is(resolver, annotation, CLASS_VAR))
                && !resolver.is_immutable_annotation(annotation);
            mutable.then(|| Report::at(PythonRule::RUF008, value, ""))
        })
        .collect()
}

fn post_init_defaults(resolver: &Resolver<'_, '_>, class: Node<'_>) -> Vec<Report> {
    let source = resolver.source;
    class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .map(|statement| {
            if statement.kind() == DECORATED {
                statement
                    .child_by_field_name(DEFINITION_FIELD)
                    .unwrap_or(statement)
            } else {
                statement
            }
        })
        .filter(|function| {
            function.kind() == FUNCTION
                && function
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == POST_INIT)
        })
        .flat_map(|function| post_init_reports(resolver, &PostInit { class, function }))
        .collect()
}

// Como ruff, los arreglos paran en el primer parámetro que no se puede arreglar: uno sin default
// detrás de otro con default sería un error de sintaxis.
fn post_init_reports(resolver: &Resolver<'_, '_>, post_init: &PostInit<'_>) -> Vec<Report> {
    let signature = Signature::of(post_init.function);
    let mut stopped_fixes = false;
    signature
        .positional
        .into_iter()
        .chain(signature.keyword_only)
        .filter(|parameter| has_default(*parameter))
        .filter_map(|parameter| {
            let default = parameter.child_by_field_name(VALUE_FIELD)?;
            let fix = if stopped_fixes {
                None
            } else {
                initvar_fix(resolver.model, resolver.source, post_init, parameter)
            };
            stopped_fixes |= fix.is_none();
            Some(Report::at(PythonRule::RUF033, default, "").with_fix(fix))
        })
        .collect()
}

// `is_enumeration` de ruff: alguna base, directa o heredada, es un `enum` de la biblioteca estándar.
pub fn is_enumeration(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    any_base(
        resolver,
        class,
        &mut |base| {
            resolver
                .qualified(without_subscript(base))
                .is_some_and(|name| ENUMS.contains(&name.as_str()))
        },
        &mut HashSet::new(),
    )
}
