mod safety;

use tree_sitter::Node;

use super::nodes::STATEMENT_BLOCK;
use super::scope::ScopeTree;
use super::{Context, report, report_with_fix};
use crate::fix::Fix;
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const VAR: &str = "var";
const LET: &str = "let";
const VARIABLE_DECLARATION: &str = "variable_declaration";
const LOOP_HEAD: &str = "for_in_statement";
const AMBIENT: &str = "ambient_declaration";
const GLOBAL: &str = "global";

#[derive(Debug, Clone, Copy)]
pub struct Declaration<'tree> {
    pub node: Node<'tree>,
    pub keyword: Node<'tree>,
    pub report_at: Node<'tree>,
}

pub fn var_declarations(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    context
        .of_kind(&[VARIABLE_DECLARATION, LOOP_HEAD])
        .filter_map(|node| declaration(node, context))
        .filter(|declaration| !is_in_global_augmentation(declaration.node))
        .map(|declaration| finding(tree, declaration, context.source))
        .collect()
}

fn declaration<'tree>(node: Node<'tree>, context: &Context<'tree, '_>) -> Option<Declaration<'tree>> {
    let keyword = if node.kind() == LOOP_HEAD {
        node.child_by_field_name("kind")?
    } else {
        let mut cursor = node.walk();
        node.children(&mut cursor).next()?
    };
    if context.text(keyword) != VAR {
        return None;
    }
    let report_at = node
        .parent()
        .filter(|parent| parent.kind() == AMBIENT)
        .unwrap_or_else(|| if node.kind() == LOOP_HEAD { keyword } else { node });
    Some(Declaration {
        node,
        keyword,
        report_at,
    })
}

fn is_in_global_augmentation(node: Node<'_>) -> bool {
    node.parent()
        .filter(|block| block.kind() == STATEMENT_BLOCK)
        .and_then(|block| block.parent())
        .is_some_and(|parent| parent.kind() == AMBIENT && super::scope::bindings::has_token(parent, GLOBAL))
}

fn finding(tree: &ScopeTree<'_>, declaration: Declaration<'_>, source: &[u8]) -> LintFinding {
    if safety::can_fix(tree, &declaration, source) {
        let fix = Fix::replace(declaration.keyword, LET, source);
        report_with_fix(EslintRule::NoVar, declaration.report_at, VAR, fix)
    } else {
        report(EslintRule::NoVar, declaration.report_at, VAR)
    }
}
