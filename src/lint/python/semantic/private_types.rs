use tree_sitter::Node;

use super::Visitor;
use super::model::{BindingId, BindingKind};
use super::qualified::{is_typing, qualified_name_from};
use super::type_hints::{subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{ASSIGNMENT, CALL, CLASS, FUNCTION_FIELD, LEFT_FIELD, RIGHT_FIELD, named};
use crate::rules::PythonRule;

const PRIVATE_PREFIX: char = '_';
const TYPE_VARIABLES: [&str; 3] = ["TypeVar", "ParamSpec", "TypeVarTuple"];
const PROTOCOL: &str = "Protocol";
const SUPERCLASSES_FIELD: &str = "superclasses";

// PYI018 (un `TypeVar`, `ParamSpec` o `TypeVarTuple` privado que nadie usa) y PYI046 (un protocolo
// privado que nadie usa). Avisan en el nombre.
pub fn unused_private_types(visitor: &Visitor<'_>, latest: &[(&str, BindingId)]) -> Vec<Report> {
    let model = &visitor.model;
    latest
        .iter()
        .filter(|(name, _)| name.starts_with(PRIVATE_PREFIX))
        .filter_map(|(_, id)| model.binding(*id))
        .filter(|binding| !binding.is_used())
        .filter_map(|binding| {
            let name = binding.site.node?;
            let qualifies = |node: Node<'_>, member: &str| {
                qualified_name_from(model, binding.scope, node, visitor.source)
                    .is_some_and(|qualified| is_typing(&qualified, member))
            };
            let unused_type_variable = binding.kind == BindingKind::Assignment
                && assigned_call(name)
                    .is_some_and(|function| TYPE_VARIABLES.iter().any(|member| qualifies(function, member)));
            let unused_protocol = binding.kind == BindingKind::Class
                && class_bases(name).into_iter().any(|base| {
                    let value = subscript_parts(base).map_or(base, |(value, _)| value);
                    qualifies(unwrapped(value), PROTOCOL)
                });
            if unused_type_variable {
                Some(Report::at(PythonRule::PYI018, name, ""))
            } else {
                unused_protocol.then(|| Report::at(PythonRule::PYI046, name, ""))
            }
        })
        .collect()
}

pub fn assigned_call(name: Node<'_>) -> Option<Node<'_>> {
    let assignment = name.parent().filter(|parent| {
        parent.kind() == ASSIGNMENT
            && parent
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|target| target.id() == name.id())
    })?;
    let value = assignment
        .child_by_field_name(RIGHT_FIELD)
        .filter(|value| value.kind() == CALL)?;
    value.child_by_field_name(FUNCTION_FIELD)
}

fn class_bases(name: Node<'_>) -> Vec<Node<'_>> {
    name.parent()
        .filter(|class| class.kind() == CLASS)
        .and_then(|class| class.child_by_field_name(SUPERCLASSES_FIELD))
        .map(named)
        .unwrap_or_default()
}
