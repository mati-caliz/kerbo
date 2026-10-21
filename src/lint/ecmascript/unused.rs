mod ambient;
mod usage;

use tree_sitter::Node;

use super::nodes::{IDENTIFIER, is_function};
use super::scope::{DefinitionKind, ScopeKind, ScopeTree, Variable, VariableId};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const EXPORT: &str = "export_statement";
const WRAPPERS: &[&str] = &["ambient_declaration", "expression_statement"];
const DIRECT_PARAMETERS: &[&str] = &["required_parameter", "optional_parameter"];

pub use ambient::{mark_ambient_declarations, mark_loop_returns};
pub use usage::is_inside;

pub fn unused_variables(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    tree.scopes
        .iter()
        .filter(|scope| scope.kind != ScopeKind::FunctionExpressionName)
        .flat_map(|scope| scope.variables.iter().copied())
        .filter(|&id| is_reported(tree, id, context.source))
        .filter_map(|id| tree.variable(id))
        .filter_map(|variable| report_position(tree, variable).map(|position| (variable, position)))
        .map(|(variable, position)| report(EslintRule::NoUnusedVars, position, &variable.name))
        .collect()
}

fn is_reported(tree: &ScopeTree<'_>, id: VariableId, source: &[u8]) -> bool {
    let Some(variable) = tree.variable(id) else {
        return false;
    };
    let Some(first) = variable.definitions.first() else {
        return false;
    };
    if variable.used || is_exported(variable) || usage::is_used(tree, variable, source) {
        return false;
    }
    if first.kind == DefinitionKind::Parameter
        && is_direct_parameter(first.name)
        && !is_after_last_used(tree, id)
    {
        return false;
    }
    let used_only_as_type = tree
        .references_of(variable)
        .any(|reference| usage::is_type_only(variable, reference));
    let imported = variable
        .definitions
        .iter()
        .any(|definition| matches!(definition.kind, DefinitionKind::ImportBinding { .. }));
    !(used_only_as_type && imported)
}

fn report_position<'tree>(tree: &ScopeTree<'tree>, variable: &Variable<'tree>) -> Option<Node<'tree>> {
    let own_scope = tree.variable_scope_of(variable.scope);
    tree.references_of(variable)
        .filter(|reference| reference.is_write() && tree.variable_scope_of(reference.from) == own_scope)
        .last()
        .map(|reference| reference.identifier)
        .or_else(|| {
            variable
                .definitions
                .iter()
                .find(|definition| definition.kind != DefinitionKind::EnumMember)
                .map(|definition| definition.name)
        })
}

fn is_exported(variable: &Variable<'_>) -> bool {
    let merged = is_merged_type_and_value(variable);
    variable.definitions.iter().any(|definition| {
        if definition.kind == DefinitionKind::Parameter {
            return false;
        }
        let node = if definition.node.kind() == super::nodes::VARIABLE_DECLARATOR {
            definition.declaration.unwrap_or(definition.node)
        } else {
            definition.node
        };
        let is_type_declaration = ["interface_declaration", "type_alias_declaration"].contains(&node.kind());
        !(merged && is_type_declaration)
            && parent_skipping_wrappers(node).is_some_and(|parent| parent.kind() == EXPORT)
    })
}

fn parent_skipping_wrappers(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent()?;
    while WRAPPERS.contains(&parent.kind()) {
        parent = parent.parent()?;
    }
    Some(parent)
}

fn is_merged_type_and_value(variable: &Variable<'_>) -> bool {
    variable.is_type_variable() && variable.is_value_variable()
}

fn is_direct_parameter(name: Node<'_>) -> bool {
    let Some(parent) = name.parent() else {
        return false;
    };
    if DIRECT_PARAMETERS.contains(&parent.kind()) {
        return parent.child_by_field_name("value").is_none()
            && parent
                .parent()
                .and_then(|list| list.parent())
                .is_some_and(is_function);
    }
    if parent.kind() == "formal_parameters" {
        return parent.parent().is_some_and(is_function);
    }
    is_function(parent) && name.kind() == IDENTIFIER
}

fn is_after_last_used(tree: &ScopeTree<'_>, id: VariableId) -> bool {
    let Some(definition) = tree
        .variable(id)
        .and_then(|variable| variable.definitions.first())
    else {
        return true;
    };
    let declared = tree.declared_by(definition.node);
    let later = declared
        .iter()
        .position(|candidate| *candidate == id)
        .map_or(&[][..], |index| {
            declared.get(index.saturating_add(1)..).unwrap_or_default()
        });
    !later
        .iter()
        .filter_map(|candidate| tree.variable(*candidate))
        .any(|variable| {
            variable.used
                || tree
                    .references_of(variable)
                    .any(|reference| !usage::is_type_only(variable, reference))
        })
}
