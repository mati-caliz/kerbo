use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::assignments::is_imported;
use super::nodes::{IDENTIFIER, NAME_FIELD, VALUE_FIELD, VARIABLE_DECLARATOR, text};
use super::scope::{Definition, DefinitionKind, Reference, ScopeTree, Variable};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const RESTRICTED: &[&str] = &["undefined", "NaN", "Infinity", "arguments", "eval", "globalThis"];
const UNDEFINED: &str = "undefined";
const KIND_FIELD: &str = "kind";
const CONST: &str = "const";
const AMBIENT: &str = "ambient_declaration";
// Las que ESLint visita como `:function`: no las firmas de tipos ni las sobrecargas.
const FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
];

// `no-shadow-restricted-names`: una declaración que tapa `undefined`, `NaN`, `Infinity`,
// `arguments`, `eval` o `globalThis`. Un `undefined` declarado sin valor y nunca escrito se perdona.
pub fn restricted_shadows(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut reported = HashSet::new();
    let mut found = Vec::new();
    for variable in tree
        .variables
        .iter()
        .filter(|variable| RESTRICTED.contains(&variable.name.as_str()))
    {
        if !is_declared_by_checked_node(variable) || safely_shadows_undefined(tree, variable) {
            continue;
        }
        for definition in &variable.definitions {
            if reported.insert(definition.name.id()) {
                found.push(report(
                    EslintRule::NoShadowRestrictedNames,
                    definition.name,
                    &variable.name,
                ));
            }
        }
    }
    found
}

fn is_declared_by_checked_node(variable: &Variable<'_>) -> bool {
    is_imported(variable) || variable.definitions.iter().any(is_checked_definition)
}

fn is_checked_definition(definition: &Definition<'_>) -> bool {
    match definition.kind {
        DefinitionKind::Variable | DefinitionKind::CatchClause | DefinitionKind::ClassName => true,
        DefinitionKind::FunctionName | DefinitionKind::Parameter => {
            FUNCTIONS.contains(&definition.node.kind())
        }
        DefinitionKind::ImportBinding { .. }
        | DefinitionKind::EnumMember
        | DefinitionKind::EnumName
        | DefinitionKind::ModuleName
        | DefinitionKind::Type => false,
    }
}

fn safely_shadows_undefined(tree: &ScopeTree<'_>, variable: &Variable<'_>) -> bool {
    variable.name == UNDEFINED
        && tree
            .references_of(variable)
            .all(|reference| !reference.is_write())
        && variable.definitions.iter().all(|definition| {
            definition.kind == DefinitionKind::Variable
                && definition.node.kind() == VARIABLE_DECLARATOR
                && definition.node.child_by_field_name(VALUE_FIELD).is_none()
        })
}

// `no-unassigned-vars`: un `let` o `var` sin valor que se lee pero nunca se escribe.
pub fn unassigned_variables(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut declared: HashMap<usize, &Variable<'_>> = HashMap::new();
    for variable in &tree.variables {
        for definition in &variable.definitions {
            declared.entry(definition.node.id()).or_insert(variable);
        }
    }
    context
        .of_kind(&[VARIABLE_DECLARATOR])
        .filter(|declarator| is_unassigned_candidate(*declarator, context.source))
        .filter_map(|declarator| {
            let variable = declared.get(&declarator.id())?;
            let never_written = tree
                .references_of(variable)
                .all(|reference| !reference.is_write());
            let read = tree.references_of(variable).any(Reference::is_read);
            (never_written && read).then(|| report(EslintRule::NoUnassignedVars, declarator, &variable.name))
        })
        .collect()
}

fn is_unassigned_candidate(declarator: Node<'_>, source: &[u8]) -> bool {
    let declaration = declarator.parent();
    declarator.child_by_field_name(VALUE_FIELD).is_none()
        && declarator
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| name.kind() == IDENTIFIER)
        && declaration
            .and_then(|declaration| declaration.child_by_field_name(KIND_FIELD))
            .is_none_or(|kind| text(kind, source) != CONST)
        && !has_ambient_ancestor(declarator)
}

fn has_ambient_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if ancestor.kind() == AMBIENT {
            return true;
        }
        current = ancestor.parent();
    }
    false
}
