mod checkstyle;
mod eslint;
mod ruff;
mod varnamelen;

use tree_sitter::Node;

use crate::profile::ShortNameStyle;
use crate::syntax::{descendants, named_children, one_based};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortName {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

pub fn find_short_names(root: Node<'_>, source: &[u8], style: ShortNameStyle) -> Vec<ShortName> {
    let nodes = descendants(root);
    match style {
        ShortNameStyle::Eslint => eslint::reported(&nodes, source),
        ShortNameStyle::Checkstyle => checkstyle::reported(&nodes, source),
        ShortNameStyle::Varnamelen => varnamelen::reported(root, source),
        ShortNameStyle::Ruff => ruff::reported(&nodes, source),
    }
}

fn short_name(position: Node<'_>, name: &str) -> ShortName {
    let start = position.start_position();
    ShortName {
        line: one_based(start.row),
        column: one_based(start.column),
        name: name.to_owned(),
    }
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

fn field<'tree>(node: Node<'tree>, name: &str) -> Option<Node<'tree>> {
    node.child_by_field_name(name)
}

fn has_token(node: Node<'_>, token: &str) -> bool {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .any(|child| child.kind() == token)
}

#[cfg(test)]
mod tests;
