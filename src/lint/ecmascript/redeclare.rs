use std::collections::HashSet;

use super::scope::{DefinitionKind, ScopeKind};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

// Los alcances que mira `no-redeclare`: el programa (global y módulo), las funciones y los bloques
// con alcance propio. Las clases, el nombre de una función con nombre, los `catch` y los tipos no.
const CHECKED_SCOPES: &[ScopeKind] = &[
    ScopeKind::Global,
    ScopeKind::Module,
    ScopeKind::Function,
    ScopeKind::Block,
    ScopeKind::For,
    ScopeKind::Switch,
    ScopeKind::ClassStaticBlock,
];

// `no-redeclare`: cada declaración de una variable después de la primera, en el mismo alcance.
pub fn redeclarations(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    tree.scopes
        .iter()
        .filter(|scope| CHECKED_SCOPES.contains(&scope.kind))
        .flat_map(|scope| scope.variables.iter())
        .filter_map(|id| tree.variable(*id))
        .flat_map(|variable| {
            variable
                .definitions
                .iter()
                .skip(1)
                .map(|definition| report(EslintRule::NoRedeclare, definition.name, &variable.name))
        })
        .collect()
}

// `no-func-assign`: lo que le asigna algo al nombre de una función declarada como función.
pub fn function_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut found = Vec::new();
    for variable in &tree.variables {
        if variable
            .definitions
            .first()
            .is_none_or(|definition| definition.kind != DefinitionKind::FunctionName)
        {
            continue;
        }
        let mut reported = HashSet::new();
        for reference in tree.references_of(variable) {
            if reference.is_write() && !reference.init && reported.insert(reference.identifier.id()) {
                found.push(report(
                    EslintRule::NoFuncAssign,
                    reference.identifier,
                    &variable.name,
                ));
            }
        }
    }
    found
}
