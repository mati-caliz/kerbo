mod groups;

use tree_sitter::Node;

use self::groups::{DestructuringHost, destructuring_host};
use super::nodes::{
    ASSIGNMENT, PARENTHESIZED, STATEMENT_BLOCK, SWITCH_CASE, SWITCH_DEFAULT, parent_without_parentheses,
};
use super::scope::{DefinitionKind, Reference, ScopeTree, Variable, VariableId};
use super::{Context, report, report_with_fix};
use crate::fix::Fix;
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const LET: &str = "let";
const CONST: &str = "const";
const LEXICAL: &str = "lexical_declaration";
const LOOP_HEAD: &str = "for_in_statement";
const FOR: &str = "for_statement";
const DECLARATION_HOSTS: &[&str] = &["program", STATEMENT_BLOCK, SWITCH_CASE, SWITCH_DEFAULT];
const PATTERN_PARENTS: &[&str] = &[
    "object_pattern",
    "array_pattern",
    "assignment_pattern",
    "object_assignment_pattern",
    "pair_pattern",
    "rest_pattern",
];

pub fn prefer_const(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let variables: Vec<VariableId> = context
        .of_kind(&[LEXICAL, LOOP_HEAD])
        .filter(|declaration| is_let(*declaration, context) && !is_for_initializer(*declaration))
        .flat_map(|declaration| tree.declared_by(declaration).iter().copied())
        .collect();
    let mut reporter = groups::Reporter::new(context.source);
    for identifiers in groups::group_by_destructuring(tree, &variables, context.source) {
        reporter.check(&identifiers);
    }
    reporter.into_findings()
}

fn is_let(declaration: Node<'_>, context: &Context<'_, '_>) -> bool {
    declaration
        .child_by_field_name("kind")
        .is_some_and(|kind| context.text(kind) == LET)
}

fn is_for_initializer(declaration: Node<'_>) -> bool {
    declaration.parent().is_some_and(|parent| {
        parent.kind() == FOR
            && parent
                .child_by_field_name("initializer")
                .is_some_and(|initializer| initializer.id() == declaration.id())
    })
}

pub fn identifier_if_should_be_const<'tree>(
    tree: &ScopeTree<'tree>,
    variable: &Variable<'tree>,
    source: &[u8],
) -> Option<Node<'tree>> {
    let mut writer: Option<&Reference<'tree>> = None;
    let mut read_before_init = false;
    for reference in tree.references_of(variable) {
        if !reference.is_write() {
            read_before_init |= reference.is_read() && writer.is_none();
            continue;
        }
        let reassigned = writer.is_some_and(|writer| writer.identifier.id() != reference.identifier.id());
        let blocked = matches!(
            destructuring_host(reference),
            Some(DestructuringHost::Assignment(assignment))
                if groups::has_outer_or_member_targets(tree, assignment, variable, source)
        );
        if reassigned || blocked {
            return None;
        }
        writer = Some(reference);
    }
    let writer = writer?;
    if writer.from != variable.scope || !can_become_declaration(writer.identifier) {
        return None;
    }
    if read_before_init {
        return variable.definitions.first().map(|definition| definition.name);
    }
    Some(writer.identifier)
}

fn can_become_declaration(identifier: Node<'_>) -> bool {
    let mut node = identifier.parent();
    while let Some(current) = node.filter(|current| PATTERN_PARENTS.contains(&current.kind())) {
        node = current.parent();
    }
    let Some(node) = node else {
        return false;
    };
    match node.kind() {
        "variable_declarator" => true,
        LOOP_HEAD => node.child_by_field_name("kind").is_some(),
        PARENTHESIZED | ASSIGNMENT => {
            let assignment = if node.kind() == PARENTHESIZED {
                parent_without_parentheses(identifier)
            } else {
                Some(node)
            };
            assignment
                .filter(|assignment| assignment.kind() == ASSIGNMENT)
                .and_then(parent_without_parentheses)
                .filter(|statement| statement.kind() == "expression_statement")
                .and_then(|statement| statement.parent())
                .is_some_and(|host| DECLARATION_HOSTS.contains(&host.kind()))
        }
        _ => false,
    }
}

fn report_identifier(identifier: Node<'_>, name: &str, fix: Option<Fix>) -> LintFinding {
    fix.map_or_else(
        || report(EslintRule::PreferConst, identifier, name),
        |fix| report_with_fix(EslintRule::PreferConst, identifier, name, fix),
    )
}

fn keyword_fix(declaration: Node<'_>, source: &[u8]) -> Option<Fix> {
    let keyword = declaration.child_by_field_name("kind")?;
    Some(Fix::replace(keyword, CONST, source))
}

fn is_parameter(variable: &Variable<'_>) -> bool {
    variable
        .definitions
        .iter()
        .any(|definition| definition.kind == DefinitionKind::Parameter)
}
