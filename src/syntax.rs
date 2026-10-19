use tree_sitter::Node;

use crate::profile::{Kinds, Profile};

const ANONYMOUS_FUNCTION: &str = "(anónima)";
const NAME_FIELD: &str = "name";
const NAMING_PARENTS: &[&str] = &[
    "type_alias_declaration",
    "variable_declarator",
    "pair",
    "public_field_definition",
    "assignment",
];

pub fn children_with_fields(node: Node<'_>) -> Vec<(Option<&str>, Node<'_>)> {
    let mut cursor = node.walk();
    let mut children = Vec::new();
    if cursor.goto_first_child() {
        loop {
            children.push((cursor.field_name(), cursor.node()));
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    children
}

pub fn logical_operator<'source>(
    profile: &Profile,
    node: Node<'_>,
    source: &'source [u8],
    operators: Kinds,
) -> Option<&'source str> {
    if !profile.logical.kinds.contains(&node.kind()) {
        return None;
    }
    let operator = node.child_by_field_name("operator")?.utf8_text(source).ok()?;
    operators.contains(&operator).then_some(operator)
}

pub fn parent_skipping_parentheses<'tree>(profile: &Profile, node: Node<'tree>) -> Option<Node<'tree>> {
    let mut parent = node.parent()?;
    while profile.parenthesized.contains(&parent.kind()) {
        parent = parent.parent()?;
    }
    Some(parent)
}

pub fn function_name(node: Node<'_>, source: &[u8]) -> String {
    let own_name = node.child_by_field_name(NAME_FIELD);
    let inherited_name = || {
        node.parent()
            .filter(|parent| NAMING_PARENTS.contains(&parent.kind()))
            .and_then(|parent| {
                parent
                    .child_by_field_name(NAME_FIELD)
                    .or_else(|| parent.child_by_field_name("left"))
            })
    };
    own_name
        .or_else(inherited_name)
        .and_then(|name| name.utf8_text(source).ok())
        .map_or_else(|| ANONYMOUS_FUNCTION.to_owned(), str::to_owned)
}

pub fn descendants(root: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        found.push(node);
        pending.extend(
            (0..node.child_count())
                .rev()
                .filter_map(|index| node.child(index)),
        );
    }
    found
}

pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub const fn one_based(position: usize) -> usize {
    position.saturating_add(1)
}
