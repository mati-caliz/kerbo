use tree_sitter::Node;

use crate::syntax::named_children;

pub const PROGRAM: &str = "program";
pub const PARENTHESIZED: &str = "parenthesized_expression";
pub const PARENTHESIZED_TYPE: &str = "parenthesized_type";
pub const COMMENT: &str = "comment";
pub const CLOSING_BRACE: &str = "}";
pub const STATEMENT_BLOCK: &str = "statement_block";
pub const IDENTIFIER: &str = "identifier";
pub const STRING: &str = "string";
pub const TEMPLATE_STRING: &str = "template_string";
pub const TEMPLATE_SUBSTITUTION: &str = "template_substitution";
pub const NUMBER: &str = "number";
pub const CALL: &str = "call_expression";
pub const ARGUMENTS: &str = "arguments";
pub const MEMBER: &str = "member_expression";
pub const SUBSCRIPT: &str = "subscript_expression";
pub const ASSIGNMENT: &str = "assignment_expression";
pub const AUGMENTED_ASSIGNMENT: &str = "augmented_assignment_expression";
pub const ASSIGNMENTS: &[&str] = &[ASSIGNMENT, AUGMENTED_ASSIGNMENT];
pub const BINARY: &str = "binary_expression";
pub const THIS: &str = "this";
pub const SWITCH_CASE: &str = "switch_case";
pub const SWITCH_DEFAULT: &str = "switch_default";
pub const IF: &str = "if_statement";
pub const ELSE: &str = "else_clause";
pub const VARIABLE_DECLARATOR: &str = "variable_declarator";
pub const FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
];
pub const NAME_FIELD: &str = "name";
pub const VALUE_FIELD: &str = "value";
pub const BODY_FIELD: &str = "body";
pub const CONDITION_FIELD: &str = "condition";
pub const LEFT_FIELD: &str = "left";
pub const RIGHT_FIELD: &str = "right";
pub const OPERATOR_FIELD: &str = "operator";
pub const FUNCTION_FIELD: &str = "function";
pub const ARGUMENTS_FIELD: &str = "arguments";
pub const PROPERTY_FIELD: &str = "property";
pub const INDEX_FIELD: &str = "index";
pub const KEY_FIELD: &str = "key";
pub const OBJECT_FIELD: &str = "object";

pub fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

pub fn is_function(node: Node<'_>) -> bool {
    node.is_named() && FUNCTIONS.contains(&node.kind())
}

pub fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
}

pub fn without_parentheses(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match first_named_child(current) {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}

pub fn parenthesis_layers(node: Node<'_>) -> usize {
    let mut layers = 0_usize;
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        layers = layers.saturating_add(1);
        match first_named_child(current) {
            Some(inner) => current = inner,
            None => break,
        }
    }
    layers
}

pub fn parent_without_parentheses(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    Some(parent)
}

pub fn enclosed_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .take_while(|child| child.kind() != CLOSING_BRACE)
        .filter(Node::is_named)
        .collect()
}

pub fn is_assignment(node: Node<'_>) -> bool {
    ASSIGNMENTS.contains(&node.kind())
}

pub fn field_children<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

pub fn operator<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.child_by_field_name(OPERATOR_FIELD)
        .map_or("", |operator| text(operator, source))
}
