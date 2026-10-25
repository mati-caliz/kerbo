use tree_sitter::Node;

use super::Report;
use super::ast::{
    BLOCK, CLASS, COMMENT, CONCATENATED_STRING, EXPRESSION_STATEMENT, FUNCTION, INTERPOLATION, MODULE,
    STRING, is_f_string, named, string_prefix, text,
};
use crate::fix::Fix;
use crate::rules::PythonRule;

const ESCAPED_OPENING: &str = "{{";
const ESCAPED_CLOSING: &str = "}}";

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == STRING && is_f_string(**node, source) && !has_placeholder(**node))
        .filter(|node| {
            node.parent()
                .filter(|parent| parent.kind() == CONCATENATED_STRING)
                .is_none_or(|concatenation| !named(concatenation).into_iter().any(has_placeholder))
        })
        .map(|node| {
            let fix = Fix::replace(*node, &plain(*node, source), source);
            let fix = if in_docstring_position(*node) {
                fix.marked_unsafe()
            } else {
                fix
            };
            Report::at(PythonRule::F541, *node, "").with_fix(Some(fix))
        })
        .collect()
}

// Como ruff, sacarle la `f` a un docstring es inseguro: pasa a ser el docstring de verdad.
fn in_docstring_position(string: Node<'_>) -> bool {
    let expression = string
        .parent()
        .filter(|parent| parent.kind() == CONCATENATED_STRING)
        .unwrap_or(string);
    let Some(statement) = expression
        .parent()
        .filter(|parent| parent.kind() == EXPRESSION_STATEMENT && parent.named_child_count() == 1)
    else {
        return false;
    };
    let Some(container) = statement.parent() else {
        return false;
    };
    let documented = container.kind() == MODULE
        || (container.kind() == BLOCK
            && container
                .parent()
                .is_some_and(|owner| matches!(owner.kind(), FUNCTION | CLASS)));
    documented
        && named(container)
            .into_iter()
            .find(|child| child.kind() != COMMENT)
            .is_some_and(|first| first.id() == statement.id())
}

fn has_placeholder(string: Node<'_>) -> bool {
    named(string).iter().any(|part| part.kind() == INTERPOLATION)
}

fn plain(string: Node<'_>, source: &[u8]) -> String {
    let prefix = string_prefix(string, source);
    let literal = text(string, source);
    let body = literal.get(prefix.len()..).unwrap_or_default();
    let kept: String = prefix
        .chars()
        .filter(|letter| !matches!(letter, 'f' | 'F'))
        .collect();
    format!(
        "{kept}{}",
        body.replace(ESCAPED_OPENING, "{").replace(ESCAPED_CLOSING, "}")
    )
}
