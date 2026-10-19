mod checkstyle;
mod eslint;
mod mnd;
mod number;
mod ruff;

use tree_sitter::Node;

use crate::profile::MagicNumberStyle;
use crate::syntax::{descendants, named_children, one_based};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicNumber {
    pub line: usize,
    pub column: usize,
    pub text: String,
}

pub fn find_magic_numbers(root: Node<'_>, source: &[u8], style: MagicNumberStyle) -> Vec<MagicNumber> {
    let nodes = descendants(root);
    let reported = match style {
        MagicNumberStyle::Eslint => nodes
            .into_iter()
            .filter_map(|node| eslint::reported(node, source))
            .collect(),
        MagicNumberStyle::Checkstyle => nodes
            .into_iter()
            .filter_map(|node| checkstyle::reported(node, source))
            .collect(),
        MagicNumberStyle::Mnd => mnd::reported(&nodes, source),
        MagicNumberStyle::Ruff => nodes
            .into_iter()
            .flat_map(|node| ruff::reported(node, source))
            .collect(),
    };
    reported
        .into_iter()
        .map(|node| magic_number(node, source))
        .collect()
}

fn magic_number(node: Node<'_>, source: &[u8]) -> MagicNumber {
    let start = node.start_position();
    MagicNumber {
        line: one_based(start.row),
        column: one_based(start.column),
        text: node.utf8_text(source).unwrap_or_default().to_owned(),
    }
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

fn field_is(parent: Node<'_>, field: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|child| child.id() == node.id())
}

fn operator<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.child_by_field_name("operator")
        .map_or("", |operator| text(operator, source))
}

#[cfg(test)]
mod tests;
