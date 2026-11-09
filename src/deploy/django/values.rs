use tree_sitter::Node;

use crate::deploy::python_calls::string_value;
use crate::syntax::named_children;

const COMMENT: &str = "comment";
const PARENTHESIZED: &str = "parenthesized_expression";
const SEQUENCES: &[&str] = &["list", "tuple", "set"];
const RAW_LITERALS: &[&str] = &["integer", "float", "unary_operator", "true", "false"];
const STRINGS: &[&str] = &["string", "concatenated_string"];
const QUOTE: &str = "'";
const ESCAPED_QUOTE: &str = "''";
const SQL_TRUE: &str = "true";
const SQL_FALSE: &str = "false";

pub fn text(node: Node<'_>, source: &[u8]) -> Option<String> {
    string_value(node, source)
}

pub fn raw(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}

pub fn elements(node: Node<'_>) -> Vec<Node<'_>> {
    if !SEQUENCES.contains(&node.kind()) {
        return Vec::new();
    }
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn is_sequence(node: Node<'_>) -> bool {
    SEQUENCES.contains(&node.kind())
}

pub fn strings(node: Node<'_>, source: &[u8]) -> Vec<String> {
    elements(node)
        .into_iter()
        .filter_map(|element| text(element, source))
        .collect()
}

pub fn dictionary<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<(String, Node<'tree>)> {
    if node.kind() != "dictionary" {
        return Vec::new();
    }
    named_children(node)
        .into_iter()
        .filter(|pair| pair.kind() == "pair")
        .filter_map(|pair| {
            let key = text(pair.child_by_field_name("key")?, source)?;
            Some((key, pair.child_by_field_name("value")?))
        })
        .collect()
}

pub fn boolean(node: Node<'_>) -> Option<bool> {
    match node.kind() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn quoted_literal(text: &str) -> String {
    format!("{QUOTE}{}{QUOTE}", text.replace(QUOTE, ESCAPED_QUOTE))
}

pub fn sql_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "none" {
        return None;
    }
    if STRINGS.contains(&node.kind()) {
        return Some(quoted_literal(&text(node, source).unwrap_or_default()));
    }
    if RAW_LITERALS.contains(&node.kind()) {
        return Some(match boolean(node) {
            Some(true) => SQL_TRUE.to_owned(),
            Some(false) => SQL_FALSE.to_owned(),
            None => raw(node, source),
        });
    }
    Some(quoted_literal(&raw(node, source)))
}

pub fn parenthesized_content(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != PARENTHESIZED {
        return None;
    }
    named_children(node)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
}
