mod arrays;
mod assignments;
mod blocks;
mod comments;
mod comparisons;
mod constant_binary;
mod constant_condition;
mod constants;
mod duplicates;
mod escapes;
mod expressions;
mod finally;
mod members;
mod modules;
mod no_var;
mod nodes;
mod prefer_const;
mod presets;
mod redeclare;
mod repair;
mod restricted;
mod scope;
mod self_assign;
mod spread;
mod statements;
mod strings;
pub mod taint;
mod tokens;
mod triple_slash;
mod typescript;
mod unused;
mod wrappers;

use std::cell::OnceCell;

use tree_sitter::Node;

use self::scope::ScopeTree;
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::limits::Limits;
use crate::rules::{EslintRule, RuleId};
use crate::syntax::descendants;

pub use presets::relaxed_by;

type Check = fn(&Context<'_, '_>) -> Vec<LintFinding>;

const CHECKS: &[(EslintRule, Check)] = &[
    (EslintRule::NoDebugger, statements::debugger),
    (EslintRule::NoEmpty, statements::empty_blocks),
    (EslintRule::NoCaseDeclarations, statements::case_declarations),
    (EslintRule::NoCondAssign, statements::conditional_assignments),
    (EslintRule::NoPrototypeBuiltins, statements::prototype_builtins),
    (EslintRule::NoExplicitAny, typescript::explicit_any),
    (EslintRule::NoNonNullAssertion, typescript::non_null_assertions),
    (EslintRule::NoThisAlias, typescript::this_aliases),
    (EslintRule::NoEmptyObjectType, typescript::empty_object_types),
    (EslintRule::BanTsComment, comments::ts_comments),
    (EslintRule::NoDuplicateCase, duplicates::duplicate_cases),
    (EslintRule::NoDupeElseIf, duplicates::duplicate_else_ifs),
    (EslintRule::NoDupeKeys, duplicates::duplicate_keys),
    (EslintRule::NoUnusedVars, unused::unused_variables),
    (EslintRule::PreferConst, prefer_const::prefer_const),
    (EslintRule::NoVar, no_var::var_declarations),
    (EslintRule::NoRequireImports, modules::require_imports),
    (EslintRule::PreferRestParams, modules::rest_parameters),
    (EslintRule::NoUnusedExpressions, expressions::unused_expressions),
    (EslintRule::NoUselessEscape, escapes::useless_escapes),
    (EslintRule::NoRedeclare, redeclare::redeclarations),
    (EslintRule::NoFuncAssign, redeclare::function_assignments),
    (EslintRule::NoWrapperObjectTypes, wrappers::wrapper_object_types),
    (EslintRule::NoUnsafeFunctionType, wrappers::unsafe_function_types),
    (
        EslintRule::TripleSlashReference,
        triple_slash::triple_slash_references,
    ),
    (EslintRule::NoUnsafeFinally, finally::unsafe_finally),
    (EslintRule::PreferSpread, spread::spread_calls),
    (
        EslintRule::NoConstantCondition,
        constant_condition::constant_conditions,
    ),
    (
        EslintRule::NoConstantBinaryExpression,
        constant_binary::constant_binary_expressions,
    ),
    (EslintRule::NoConstAssign, assignments::constant_assignments),
    (EslintRule::NoClassAssign, assignments::class_assignments),
    (EslintRule::NoImportAssign, assignments::import_assignments),
    (EslintRule::NoGlobalAssign, assignments::global_assignments),
    (
        EslintRule::NoShadowRestrictedNames,
        restricted::restricted_shadows,
    ),
    (EslintRule::NoUnassignedVars, restricted::unassigned_variables),
    (EslintRule::ValidTypeof, comparisons::invalid_typeof_comparisons),
    (EslintRule::NoEmptyPattern, arrays::empty_patterns),
    (EslintRule::NoSparseArrays, arrays::sparse_arrays),
    (EslintRule::RequireYield, blocks::generators_without_yield),
    (EslintRule::NoUselessCatch, blocks::useless_catches),
    (
        EslintRule::NoCompareNegZero,
        comparisons::negative_zero_comparisons,
    ),
    (
        EslintRule::NoAsyncPromiseExecutor,
        blocks::async_promise_executors,
    ),
    (EslintRule::UseIsnan, comparisons::nan_comparisons),
    (EslintRule::NoUnusedLabels, blocks::unused_labels),
    (EslintRule::NoSelfAssign, self_assign::self_assignments),
];

#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'source> {
    pub source: &'source [u8],
    pub declaration_file: bool,
}

pub struct Context<'tree, 'source> {
    pub source: &'source [u8],
    pub nodes: Vec<Node<'tree>>,
    pub root: Node<'tree>,
    pub declaration_file: bool,
    pub scopes: OnceCell<ScopeTree<'tree>>,
}

impl<'tree> Context<'tree, '_> {
    pub fn of_kind<'context>(
        &'context self,
        kinds: &'context [&str],
    ) -> impl Iterator<Item = Node<'tree>> + 'context {
        self.nodes
            .iter()
            .copied()
            .filter(move |node| kinds.contains(&node.kind()))
    }

    pub fn text(&self, node: Node<'_>) -> &str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    pub fn scopes(&self) -> &ScopeTree<'tree> {
        self.scopes.get_or_init(|| {
            let mut tree = ScopeTree::analyze(self.root, self.source);
            unused::mark_ambient_declarations(&mut tree, self.root, self.source, self.declaration_file);
            unused::mark_loop_returns(&mut tree, self.root, self.source);
            tree
        })
    }
}

pub fn reported(root: Node<'_>, file: SourceFile<'_>, limits: Limits) -> Vec<LintFinding> {
    let enabled = |rule: EslintRule| limits.get(RuleId::Eslint(rule)).is_some();
    if !EslintRule::ALL.into_iter().any(enabled) {
        return Vec::new();
    }
    if !root.has_error() {
        return checked(root, file, &enabled);
    }
    repair::reparsed(root, file.source)
        .map_or_else(Vec::new, |tree| checked(tree.root_node(), file, &enabled))
}

fn checked(root: Node<'_>, file: SourceFile<'_>, enabled: &dyn Fn(EslintRule) -> bool) -> Vec<LintFinding> {
    let context = Context {
        source: file.source,
        nodes: descendants(root),
        root,
        declaration_file: file.declaration_file,
        scopes: OnceCell::new(),
    };
    if context.nodes.iter().any(|node| repair::is_flow_syntax(*node)) {
        return Vec::new();
    }
    let mut findings: Vec<LintFinding> = CHECKS
        .iter()
        .filter(|(rule, _)| enabled(*rule))
        .flat_map(|(_, check)| check(&context))
        .collect();
    findings.sort_by_key(|finding| (finding.line, finding.column));
    findings
}

fn report(rule: EslintRule, position: Node<'_>, subject: &str) -> LintFinding {
    finding(LintKind::Eslint(rule), position, subject, None)
}

fn report_with_fix(rule: EslintRule, position: Node<'_>, subject: &str, fix: Fix) -> LintFinding {
    finding(LintKind::Eslint(rule), position, subject, Some(fix))
}

#[cfg(test)]
mod tests;
