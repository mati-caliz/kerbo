use std::collections::HashMap;

use tree_sitter::Node;

use super::scope::ScopeId;
use super::{Context, report, report_with_fix};
use crate::fix::Fix;
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const TYPE_IDENTIFIER: &str = "type_identifier";
const NESTED_TYPE_IDENTIFIER: &str = "nested_type_identifier";
const GENERIC_TYPE: &str = "generic_type";
const HERITAGE_CLAUSES: &[&str] = &["implements_clause", "extends_type_clause"];
const WRAPPER_TYPES: &[&str] = &["BigInt", "Boolean", "Number", "Object", "String", "Symbol"];
const FUNCTION_TYPES: &[&str] = &["Function"];

// `no-wrapper-object-types`: `String`, `Number` y compañía como tipo, cuando son los globales.
pub fn wrapper_object_types(context: &Context<'_, '_>) -> Vec<LintFinding> {
    global_type_references(context, WRAPPER_TYPES)
        .into_iter()
        .map(|reference| {
            let name = context.text(reference);
            if is_heritage(reference) {
                report(EslintRule::NoWrapperObjectTypes, reference, name)
            } else {
                let fix = Fix::replace(reference, &name.to_lowercase(), context.source);
                report_with_fix(EslintRule::NoWrapperObjectTypes, reference, name, fix)
            }
        })
        .collect()
}

// `no-unsafe-function-type`: `Function` como tipo, cuando es el global.
pub fn unsafe_function_types(context: &Context<'_, '_>) -> Vec<LintFinding> {
    global_type_references(context, FUNCTION_TYPES)
        .into_iter()
        .map(|reference| {
            report(
                EslintRule::NoUnsafeFunctionType,
                reference,
                context.text(reference),
            )
        })
        .collect()
}

// Como `isReferenceToGlobalFunction` de typescript-eslint: decide la primera referencia con ese
// nombre creada en el alcance del uso; si no hay ninguna, el nombre cuenta como global.
fn global_type_references<'tree>(context: &Context<'tree, '_>, names: &[&str]) -> Vec<Node<'tree>> {
    let tree = context.scopes();
    let mut declared_by_first: HashMap<(ScopeId, &str), bool> = HashMap::new();
    for reference in &tree.references {
        let name = context.text(reference.identifier);
        if let Some(name) = names.iter().find(|candidate| **candidate == name) {
            let declared = reference
                .resolved
                .and_then(|id| tree.variable(id))
                .is_some_and(|variable| !variable.definitions.is_empty());
            declared_by_first
                .entry((reference.from, *name))
                .or_insert(declared);
        }
    }
    tree.references
        .iter()
        .map(|reference| reference.identifier)
        .filter(|identifier| is_type_name(*identifier))
        .filter_map(|identifier| {
            let name = context.text(identifier);
            let name = names.iter().find(|candidate| **candidate == name)?;
            let scope = tree.scope_of(identifier);
            let declared = declared_by_first.get(&(scope, *name)).copied().unwrap_or(false);
            (!declared).then_some(identifier)
        })
        .collect()
}

fn is_type_name(identifier: Node<'_>) -> bool {
    identifier.kind() == TYPE_IDENTIFIER
        && identifier
            .parent()
            .is_none_or(|parent| parent.kind() != NESTED_TYPE_IDENTIFIER)
}

fn is_heritage(identifier: Node<'_>) -> bool {
    let mut parent = identifier.parent();
    if let Some(generic) = parent.filter(|parent| parent.kind() == GENERIC_TYPE) {
        parent = generic.parent();
    }
    parent.is_some_and(|parent| HERITAGE_CLAUSES.contains(&parent.kind()))
}
