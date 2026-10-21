use tree_sitter::Node;

use super::Report;
use super::ast::{CONCATENATED_STRING, INTERPOLATION, STRING, is_f_string, named, string_prefix, text};
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
            Report::at(PythonRule::F541, *node, "").with_fix(Some(fix))
        })
        .collect()
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
