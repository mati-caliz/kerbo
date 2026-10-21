use tree_sitter::Node;

use super::referencer::Referencer;
use crate::lint::ecmascript::nodes::{COMMENT, first_named_child};
use crate::syntax::named_children;

const DIRECT_IDENTIFIERS: &[&str] = &["identifier", "undefined", "shorthand_property_identifier_pattern"];
const TRANSPARENT: &[&str] = &["non_null_expression", "parenthesized_expression", "rest_pattern"];
const PARAMETERS: &[&str] = &["required_parameter", "optional_parameter"];

#[derive(Debug, Clone, Copy)]
pub struct PatternIdentifier<'tree> {
    pub node: Node<'tree>,
    pub defaults: usize,
}

#[derive(Debug, Default)]
pub struct PatternParts<'tree> {
    pub identifiers: Vec<PatternIdentifier<'tree>>,
    pub right_hand: Vec<Node<'tree>>,
}

pub fn pattern_parts(pattern: Node<'_>) -> PatternParts<'_> {
    let mut parts = PatternParts::default();
    collect(pattern, 0, &mut parts);
    parts
}

fn collect<'tree>(node: Node<'tree>, defaults: usize, parts: &mut PatternParts<'tree>) {
    let kind = node.kind();
    if DIRECT_IDENTIFIERS.contains(&kind) {
        parts.identifiers.push(PatternIdentifier { node, defaults });
    } else if TRANSPARENT.contains(&kind) {
        if let Some(inner) = first_named_child(node) {
            collect(inner, defaults, parts);
        }
    } else if PARAMETERS.contains(&kind) {
        collect_parameter(node, defaults, parts);
    } else {
        collect_structured(node, defaults, parts);
    }
}

fn collect_structured<'tree>(node: Node<'tree>, defaults: usize, parts: &mut PatternParts<'tree>) {
    match node.kind() {
        "object_pattern" | "array_pattern" => {
            for element in named_children(node)
                .into_iter()
                .filter(|child| child.kind() != COMMENT)
            {
                collect(element, defaults, parts);
            }
        }
        "pair_pattern" => {
            if let Some(key) = node
                .child_by_field_name("key")
                .filter(|key| key.kind() == "computed_property_name")
                .and_then(first_named_child)
            {
                parts.right_hand.push(key);
            }
            if let Some(value) = node.child_by_field_name("value") {
                collect(value, defaults, parts);
            }
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect(left, defaults.saturating_add(1), parts);
            }
            parts.right_hand.extend(node.child_by_field_name("right"));
        }
        "member_expression" => parts.right_hand.extend(node.child_by_field_name("object")),
        "subscript_expression" => {
            parts.right_hand.extend(node.child_by_field_name("index"));
            parts.right_hand.extend(node.child_by_field_name("object"));
        }
        _ => {}
    }
}

fn collect_parameter<'tree>(node: Node<'tree>, defaults: usize, parts: &mut PatternParts<'tree>) {
    let Some(pattern) = node
        .child_by_field_name("pattern")
        .filter(|pattern| pattern.kind() != "this")
    else {
        return;
    };
    match node.child_by_field_name("value") {
        Some(value) => {
            collect(pattern, defaults.saturating_add(1), parts);
            parts.right_hand.push(value);
        }
        None => collect(pattern, defaults, parts),
    }
}

pub trait PatternVisitor<'tree> {
    fn declare_pattern(&mut self, pattern: Node<'tree>, callback: impl FnMut(&mut Self, Node<'tree>, usize));
}

impl<'tree> PatternVisitor<'tree> for Referencer<'tree, '_, '_> {
    fn declare_pattern(
        &mut self,
        pattern: Node<'tree>,
        mut callback: impl FnMut(&mut Self, Node<'tree>, usize),
    ) {
        let parts = pattern_parts(pattern);
        for identifier in parts.identifiers {
            callback(self, identifier.node, identifier.defaults);
        }
        for node in parts.right_hand {
            self.visit(node);
        }
    }
}
