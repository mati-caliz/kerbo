use tree_sitter::Node;

use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, CALL, FUNCTION_FIELD, IDENTIFIER, MEMBER, OBJECT_FIELD,
    parent_without_parentheses, without_parentheses,
};
use super::scope::ScopeKind;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const REQUIRE: &str = "require";
const REQUIRE_CLAUSE: &str = "import_require_clause";
const IMPLICIT_ARGUMENTS: &str = "arguments";
const NON_ARROW_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
];

pub fn require_imports(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let calls = context
        .of_kind(&[CALL])
        .filter(|call| {
            call.child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| arguments.kind() == ARGUMENTS)
        })
        .filter(|call| {
            call.child_by_field_name(FUNCTION_FIELD)
                .map(without_parentheses)
                .is_some_and(|callee| callee.kind() == IDENTIFIER && context.text(callee) == REQUIRE)
        })
        .filter(|call| {
            tree.find(tree.scope_of(*call), REQUIRE)
                .and_then(|id| tree.variable(id))
                .is_none_or(|variable| variable.definitions.is_empty())
        })
        .map(|call| report(EslintRule::NoRequireImports, call, REQUIRE));
    let clauses = context
        .of_kind(&[REQUIRE_CLAUSE])
        .filter_map(require_keyword)
        .map(|keyword| report(EslintRule::NoRequireImports, keyword, REQUIRE));
    calls.chain(clauses).collect()
}

fn require_keyword(clause: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = clause.walk();
    clause
        .children(&mut cursor)
        .find(|child| !child.is_named() && child.kind() == REQUIRE)
}

pub fn rest_parameters(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    tree.scopes
        .iter()
        .filter(|scope| {
            scope.kind == ScopeKind::Function && NON_ARROW_FUNCTIONS.contains(&scope.block.kind())
        })
        .filter_map(|scope| scope.lookup(IMPLICIT_ARGUMENTS))
        .filter_map(|id| tree.variable(id))
        .filter(|variable| variable.definitions.is_empty())
        .flat_map(|variable| tree.references_of(variable))
        .filter(|reference| !is_member_access(reference.identifier))
        .map(|reference| {
            report(
                EslintRule::PreferRestParams,
                reference.identifier,
                IMPLICIT_ARGUMENTS,
            )
        })
        .collect()
}

fn is_member_access(identifier: Node<'_>) -> bool {
    parent_without_parentheses(identifier).is_some_and(|parent| {
        parent.kind() == MEMBER
            && parent
                .child_by_field_name(OBJECT_FIELD)
                .map(without_parentheses)
                .is_some_and(|object| object.id() == identifier.id())
    })
}
