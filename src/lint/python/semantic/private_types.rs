use tree_sitter::Node;

use super::Visitor;
use super::model::{BindingId, BindingKind, ScopeKind};
use super::qualified::{is_typing, qualified_name_from};
use super::type_hints::{subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, CALL, CLASS, FUNCTION_FIELD, LEFT_FIELD, RIGHT_FIELD, named, text,
};
use crate::rules::PythonRule;

const PRIVATE_PREFIX: char = '_';
const TYPE_VARIABLES: [&str; 3] = ["TypeVar", "ParamSpec", "TypeVarTuple"];
const PROTOCOL: &str = "Protocol";
const SUPERCLASSES_FIELD: &str = "superclasses";
const TYPED_DICT: &str = "TypedDict";
const TYPE_ALIAS: &str = "TypeAlias";
const TYPE_ALIAS_STATEMENT: &str = "type_alias_statement";
const TYPE_FIELD: &str = "type";

// PYI018 (un `TypeVar`, `ParamSpec` o `TypeVarTuple` privado que nadie usa), PYI046 (un protocolo),
// PYI047 (un alias de tipo) y PYI049 (un `TypedDict`). Avisan en el nombre. Fuera de un stub, PYI047 y
// PYI049 sólo miran el módulo y las funciones.
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
            let scope_kind = model.scope_of(binding.scope).kind;
            let checked = visitor.file.stub || matches!(scope_kind, ScopeKind::Module | ScopeKind::Function);
            let unused_alias = checked
                && binding.kind == BindingKind::Assignment
                && is_type_alias(binding.site.statement, name, &qualifies);
            let typed_dict_definition = if binding.kind == BindingKind::Class {
                class_bases(name)
                    .into_iter()
                    .any(|base| qualifies(base, TYPED_DICT))
            } else {
                binding.kind == BindingKind::Assignment
                    && assigned_call(name).is_some_and(|function| qualifies(function, TYPED_DICT))
            };
            let unused_typed_dict = checked && typed_dict_definition;
            let text = text(name, visitor.source);
            if unused_type_variable {
                Some(Report::at(PythonRule::PYI018, name, ""))
            } else if unused_alias {
                Some(Report::at(PythonRule::PYI047, name, text))
            } else if unused_typed_dict {
                Some(Report::at(PythonRule::PYI049, name, text))
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

// `x: TypeAlias = ...` o `type x = ...`.
fn is_type_alias(
    statement: Option<Node<'_>>,
    name: Node<'_>,
    qualifies: &dyn Fn(Node<'_>, &str) -> bool,
) -> bool {
    if statement.is_some_and(|statement| statement.kind() == TYPE_ALIAS_STATEMENT) {
        return true;
    }
    name.parent()
        .filter(|parent| {
            parent.kind() == ASSIGNMENT
                && parent
                    .child_by_field_name(LEFT_FIELD)
                    .is_some_and(|target| target.id() == name.id())
        })
        .and_then(|assignment| assignment.child_by_field_name(TYPE_FIELD))
        .is_some_and(|annotation| qualifies(unwrapped(annotation), TYPE_ALIAS))
}
