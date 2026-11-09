use tree_sitter::Node;

use super::values::parenthesized_content;
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const ATTRIBUTE: &str = "attribute";
const CLASS_DEFINITION: &str = "class_definition";
const ASSIGNMENT: &str = "assignment";
const ENUM_VALUE: &str = "value";
const MAX_ALIAS_DEPTH: usize = 8;

pub type Bindings<'tree> = Vec<(String, Node<'tree>)>;

#[derive(Debug, Clone, Copy)]
pub struct Constants<'tree> {
    pub source: &'tree [u8],
    pub bindings: &'tree [(String, Node<'tree>)],
}

impl<'tree> Constants<'tree> {
    pub const fn new(source: &'tree [u8], bindings: &'tree [(String, Node<'tree>)]) -> Self {
        Self { source, bindings }
    }

    pub fn resolve(self, node: Node<'tree>) -> Node<'tree> {
        let mut current = node;
        for _ in 0..MAX_ALIAS_DEPTH {
            let Some(bound) = self.binding(current).or_else(|| parenthesized_content(current)) else {
                return current;
            };
            current = bound;
        }
        current
    }

    fn binding(self, node: Node<'tree>) -> Option<Node<'tree>> {
        let reference = match node.kind() {
            IDENTIFIER => node,
            ATTRIBUTE => self.enum_member(node).unwrap_or(node),
            _ => return None,
        };
        self.lookup(reference.utf8_text(self.source).ok()?)
    }

    fn enum_member(self, node: Node<'tree>) -> Option<Node<'tree>> {
        let attribute = node
            .child_by_field_name("attribute")?
            .utf8_text(self.source)
            .ok()?;
        let object = node.child_by_field_name("object")?;
        (attribute == ENUM_VALUE && self.lookup(object.utf8_text(self.source).ok()?).is_some())
            .then_some(object)
    }

    fn lookup(self, name: &str) -> Option<Node<'tree>> {
        self.bindings
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| *value)
    }
}

pub fn assignments<'tree>(body: Node<'tree>, source: &[u8]) -> Bindings<'tree> {
    named_children(body)
        .into_iter()
        .filter_map(|statement| named_children(statement).into_iter().next())
        .filter(|assignment| assignment.kind() == ASSIGNMENT)
        .filter_map(|assignment| {
            let name = assignment.child_by_field_name("left")?.utf8_text(source).ok()?;
            Some((name.to_owned(), assignment.child_by_field_name("right")?))
        })
        .collect()
}

pub fn class_attributes<'tree>(root: Node<'tree>, source: &[u8]) -> Bindings<'tree> {
    named_children(root)
        .into_iter()
        .filter(|node| node.kind() == CLASS_DEFINITION)
        .filter_map(|class| {
            let name = class.child_by_field_name("name")?.utf8_text(source).ok()?;
            let body = class.child_by_field_name("body")?;
            Some(
                assignments(body, source)
                    .into_iter()
                    .map(|(attribute, value)| (format!("{name}.{attribute}"), value))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}
