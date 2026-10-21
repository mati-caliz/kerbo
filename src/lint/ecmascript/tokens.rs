use tree_sitter::Node;

use super::nodes::{BINARY, COMMENT, LEFT_FIELD, RIGHT_FIELD, operator, text, without_parentheses};

pub const AND: &str = "&&";
pub const OR: &str = "||";
const NULLISH: &str = "??";
const LOGICAL: &str = "logical_expression";

pub fn estree_kind<'tree>(node: Node<'tree>, source: &[u8]) -> &'tree str {
    let node = without_parentheses(node);
    if node.kind() == BINARY && [AND, OR, NULLISH].contains(&operator(node, source)) {
        LOGICAL
    } else {
        node.kind()
    }
}

pub fn logical_operands<'tree>(
    node: Node<'tree>,
    wanted: &str,
    source: &[u8],
) -> Option<(Node<'tree>, Node<'tree>)> {
    let node = without_parentheses(node);
    if node.kind() != BINARY || operator(node, source) != wanted {
        return None;
    }
    Some((
        without_parentheses(node.child_by_field_name(LEFT_FIELD)?),
        without_parentheses(node.child_by_field_name(RIGHT_FIELD)?),
    ))
}

pub fn equal_tokens(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let left = without_parentheses(left);
    let right = without_parentheses(right);
    let left_tokens = leaves(left).into_iter().map(|leaf| text(leaf, source));
    let right_tokens = leaves(right).into_iter().map(|leaf| text(leaf, source));
    left_tokens.eq(right_tokens)
}

fn leaves(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == COMMENT {
            continue;
        }
        if current.child_count() == 0 {
            found.push(current);
            continue;
        }
        let mut cursor = current.walk();
        let children: Vec<Node<'_>> = current.children(&mut cursor).collect();
        pending.extend(children.iter().rev().copied());
    }
    found
}
