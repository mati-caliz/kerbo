use std::collections::HashSet;

use tree_sitter::Node;

use super::Pending;
use super::model::{BindingKind, MODULE_SCOPE, Model, Resolution};
use super::mutability::{Resolver, unwrap_type, without_subscript};
use super::qualified::is_typing;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, BODY_FIELD, CALL, CLASS, COMMENT, DECORATED, EXPRESSION_STATEMENT, FUNCTION_FIELD,
    IDENTIFIER, KEYWORD_ARGUMENT, LEFT_FIELD, RIGHT_FIELD, TYPE_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const SPECIAL_ATTRIBUTES: [&str; 4] = ["__slots__", "__dict__", "__weakref__", "__annotations__"];
const TYPING_MODULES: [&str; 3] = ["typing", "typing_extensions", "_typeshed"];
const DATACLASS_MODULES: [&str; 3] = ["dataclasses", "attr", "attrs"];
const CLASS_VAR: &str = "ClassVar";
const FINAL: &str = "Final";
const DATACLASS_DECORATORS: [&str; 10] = [
    "dataclasses.dataclass",
    "attrs.define",
    "attrs.frozen",
    "attrs.mutable",
    "attr.define",
    "attr.frozen",
    "attr.mutable",
    "attr.s",
    "attr.attributes",
    "attr.attrs",
];
const COPYING_BASES: [&str; 12] = [
    "pydantic.BaseModel",
    "pydantic.RootModel",
    "pydantic.BaseSettings",
    "pydantic.BaseConfig",
    "pydantic.generics.GenericModel",
    "pydantic.v1.BaseModel",
    "pydantic.v1.BaseSettings",
    "pydantic.v1.BaseConfig",
    "pydantic.v1.generics.GenericModel",
    "pydantic_settings.BaseSettings",
    "msgspec.Struct",
    "sqlmodel.SQLModel",
];
const CTYPES_BASES: [&str; 6] = [
    "ctypes.Structure",
    "ctypes.BigEndianStructure",
    "ctypes.LittleEndianStructure",
    "ctypes.Union",
    "ctypes.BigEndianUnion",
    "ctypes.LittleEndianUnion",
];
const CTYPES_FIELDS: &str = "_fields_";
const SUPERCLASSES_FIELD: &str = "superclasses";
const DEFINITION_FIELD: &str = "definition";
const DECORATOR: &str = "decorator";
const GENERIC_TYPE: &str = "generic_type";
const MODULE_SEPARATOR: char = '.';
const DICTIONARY_SPLAT: &str = "dictionary_splat";

// Los módulos importados en algún lugar del archivo, como `seen_module` de ruff.
struct SeenModules {
    typing: bool,
    dataclasses: bool,
}

// RUF012: un atributo de clase con un valor mutable, que comparten todas las instancias. Corre al
// final; los nombres se resuelven desde donde está la clase, y las bases heredadas desde el módulo,
// como los scopes diferidos de ruff.
pub fn mutable_class_defaults(model: &Model<'_>, source: &[u8], classes: &[Pending<'_>]) -> Vec<Report> {
    let imported: Vec<&str> = model
        .bindings
        .iter()
        .filter(|binding| binding.kind.is_import())
        .map(|binding| {
            binding
                .qualified
                .split(MODULE_SEPARATOR)
                .next()
                .unwrap_or_default()
        })
        .collect();
    let seen = SeenModules {
        typing: imported.iter().any(|module| TYPING_MODULES.contains(module)),
        dataclasses: imported.iter().any(|module| DATACLASS_MODULES.contains(module)),
    };
    classes
        .iter()
        .flat_map(|class| {
            let resolver = Resolver {
                model,
                source,
                scope: class.scope,
            };
            class_reports(&resolver, &seen, class.node)
        })
        .collect()
}

struct Assignment<'tree> {
    targets: Vec<Node<'tree>>,
    annotation: Option<Node<'tree>>,
    value: Node<'tree>,
}

fn class_reports(resolver: &Resolver<'_, '_>, seen: &SeenModules, class: Node<'_>) -> Vec<Report> {
    let mut reports = Vec::new();
    let mut class_variables: HashSet<&str> = HashSet::new();
    let source = resolver.source;
    let is_dataclass = seen.dataclasses && has_dataclass_decorator(resolver, class);
    for assignment in assignments(class) {
        let value = assignment.value;
        let Some(annotation) = assignment.annotation else {
            let only_class_variables = assignment.targets.iter().all(|target| {
                is_special_attribute(*target, source)
                    || (target.kind() == IDENTIFIER && class_variables.contains(text(*target, source)))
            });
            if only_class_variables || !resolver.is_mutable_expression(value) {
                continue;
            }
            if is_ctypes_fields(resolver, class, &assignment.targets) || copies_defaults(resolver, class) {
                return reports;
            }
            reports.push(Report::at(PythonRule::RUF012, unparenthesized(value), ""));
            continue;
        };
        let is_class_variable = seen.typing && annotation_is(resolver, annotation, CLASS_VAR);
        if let [target] = assignment.targets.as_slice()
            && target.kind() == IDENTIFIER
            && is_class_variable
        {
            class_variables.insert(text(*target, source));
        }
        let is_final = seen.typing && annotation_is(resolver, annotation, FINAL);
        let candidate = !assignment
            .targets
            .iter()
            .any(|target| is_special_attribute(*target, source))
            && resolver.is_mutable_expression(value)
            && !is_class_variable
            && !is_final
            && !resolver.is_immutable_annotation(annotation);
        if !candidate || is_dataclass {
            continue;
        }
        if copies_defaults(resolver, class) {
            return reports;
        }
        reports.push(Report::at(PythonRule::RUF012, unparenthesized(value), ""));
    }
    reports
}

fn assignments(class: Node<'_>) -> Vec<Assignment<'_>> {
    class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|statement| statement.kind() == EXPRESSION_STATEMENT)
        .filter_map(|statement| {
            named(statement)
                .into_iter()
                .find(|child| child.kind() == ASSIGNMENT)
        })
        .filter_map(|assignment| {
            let annotation = assignment.child_by_field_name(TYPE_FIELD);
            let mut targets = Vec::new();
            let mut current = assignment;
            loop {
                targets.extend(current.child_by_field_name(LEFT_FIELD));
                match current.child_by_field_name(RIGHT_FIELD) {
                    Some(next) if next.kind() == ASSIGNMENT => current = next,
                    value => {
                        return value.map(|value| Assignment {
                            targets,
                            annotation,
                            value,
                        });
                    }
                }
            }
        })
        .collect()
}

fn is_special_attribute(target: Node<'_>, source: &[u8]) -> bool {
    target.kind() == IDENTIFIER && SPECIAL_ATTRIBUTES.contains(&text(target, source))
}

// `ClassVar` o `Final`, solos o con su tipo entre corchetes.
fn annotation_is(resolver: &Resolver<'_, '_>, annotation: Node<'_>, member: &str) -> bool {
    let annotation = unwrap_type(annotation);
    let head = if annotation.kind() == GENERIC_TYPE {
        named(annotation).first().copied().unwrap_or(annotation)
    } else {
        without_subscript(annotation)
    };
    resolver
        .qualified(head)
        .is_some_and(|name| is_typing(&name, member))
}

fn has_dataclass_decorator(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    class
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .filter_map(|decorator| named(decorator).first().copied())
        .map(|expression| {
            if expression.kind() == CALL {
                expression
                    .child_by_field_name(FUNCTION_FIELD)
                    .unwrap_or(expression)
            } else {
                expression
            }
        })
        .any(|decorator| {
            resolver
                .qualified(decorator)
                .is_some_and(|name| DATACLASS_DECORATORS.contains(&name.as_str()))
        })
}

// `has_default_copy_semantics`: modelos de pydantic, msgspec o sqlmodel, que copian los defaults.
fn copies_defaults(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    any_base_named(resolver, class, &COPYING_BASES, &mut HashSet::new())
}

fn is_ctypes_fields(resolver: &Resolver<'_, '_>, class: Node<'_>, targets: &[Node<'_>]) -> bool {
    let is_fields = targets
        .first()
        .is_some_and(|target| target.kind() == IDENTIFIER && text(*target, resolver.source) == CTYPES_FIELDS);
    is_fields && any_base_named(resolver, class, &CTYPES_BASES, &mut HashSet::new())
}

// `any_qualified_base_class`: alguna base, directa o heredada de una clase del archivo, se llama así.
fn any_base_named(
    resolver: &Resolver<'_, '_>,
    class: Node<'_>,
    names: &[&str],
    seen: &mut HashSet<usize>,
) -> bool {
    bases(class).into_iter().any(|base| {
        let base = without_subscript(base);
        if resolver
            .qualified(base)
            .is_some_and(|name| names.contains(&name.as_str()))
        {
            return true;
        }
        if base.kind() != IDENTIFIER {
            return false;
        }
        let Resolution::Binding(id) = resolver
            .model
            .lookup_from(MODULE_SCOPE, text(base, resolver.source))
        else {
            return false;
        };
        let parent = resolver
            .model
            .binding(id)
            .filter(|binding| binding.kind == BindingKind::Class)
            .and_then(|binding| binding.site.statement)
            .map(|statement| {
                statement
                    .child_by_field_name(DEFINITION_FIELD)
                    .unwrap_or(statement)
            })
            .filter(|statement| statement.kind() == CLASS);
        parent.is_some_and(|parent| seen.insert(id) && any_base_named(resolver, parent, names, seen))
    })
}

fn bases(class: Node<'_>) -> Vec<Node<'_>> {
    class
        .child_by_field_name(SUPERCLASSES_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|base| !matches!(base.kind(), COMMENT | KEYWORD_ARGUMENT | DICTIONARY_SPLAT))
        .collect()
}
