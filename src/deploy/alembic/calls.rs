use tree_sitter::Node;

use crate::syntax::{named_children, one_based};

#[derive(Debug, Clone)]
pub struct Call<'tree> {
    pub receiver: &'tree str,
    pub method: &'tree str,
    pub line: usize,
    pub column: usize,
    pub arguments: Vec<Node<'tree>>,
    pub source: &'tree [u8],
}

impl<'tree> Call<'tree> {
    pub fn parse(node: Node<'tree>, source: &'tree [u8]) -> Option<Self> {
        if node.kind() != "call" {
            return None;
        }
        let function = node.child_by_field_name("function")?;
        let (receiver, method) = match function.kind() {
            "attribute" => (
                function.child_by_field_name("object")?.utf8_text(source).ok()?,
                function
                    .child_by_field_name("attribute")?
                    .utf8_text(source)
                    .ok()?,
            ),
            "identifier" => ("", function.utf8_text(source).ok()?),
            _ => return None,
        };
        let arguments = node
            .child_by_field_name("arguments")
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|argument| argument.kind() != "comment")
            .collect();
        Some(Self {
            receiver,
            method,
            line: one_based(node.start_position().row),
            column: one_based(node.start_position().column),
            arguments,
            source,
        })
    }

    pub fn all_positional(&self) -> Vec<Node<'tree>> {
        self.arguments
            .iter()
            .copied()
            .filter(|argument| argument.kind() != "keyword_argument")
            .collect()
    }

    pub fn positional(&self, index: usize) -> Option<Node<'tree>> {
        self.all_positional().get(index).copied()
    }

    pub fn keyword(&self, name: &str) -> Option<Node<'tree>> {
        self.arguments
            .iter()
            .filter(|argument| argument.kind() == "keyword_argument")
            .find(|argument| {
                argument
                    .child_by_field_name("name")
                    .and_then(|key| key.utf8_text(self.source).ok())
                    == Some(name)
            })
            .and_then(|argument| argument.child_by_field_name("value"))
    }

    pub fn keyword_is_true(&self, name: &str) -> bool {
        self.keyword(name).is_some_and(|value| value.kind() == "true")
    }

    pub fn text_argument(&self, index: usize, name: &str) -> Option<String> {
        self.positional(index)
            .or_else(|| self.keyword(name))
            .and_then(|node| string_value(node, self.source))
    }
}

pub fn string_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "string" => Some(
            named_children(node)
                .into_iter()
                .filter(|part| part.kind() == "string_content")
                .filter_map(|part| part.utf8_text(source).ok())
                .collect(),
        ),
        "concatenated_string" => Some(
            named_children(node)
                .into_iter()
                .filter_map(|part| string_value(part, source))
                .collect(),
        ),
        "parenthesized_expression" => named_children(node)
            .into_iter()
            .find_map(|inner| string_value(inner, source)),
        _ => None,
    }
}
