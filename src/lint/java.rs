use tree_sitter::Node;

use crate::syntax::named_children;

pub const PROGRAM: &str = "program";
pub const MODIFIERS: &str = "modifiers";
pub const CLASS: &str = "class_declaration";
pub const INTERFACE: &str = "interface_declaration";
pub const ENUM: &str = "enum_declaration";
pub const RECORD: &str = "record_declaration";
pub const ANNOTATION_TYPE: &str = "annotation_type_declaration";
pub const TYPE_DECLARATIONS: [&str; 5] = [CLASS, INTERFACE, ENUM, RECORD, ANNOTATION_TYPE];
pub const METHOD: &str = "method_declaration";
pub const CONSTRUCTOR: &str = "constructor_declaration";
pub const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
pub const ENUM_CONSTANT: &str = "enum_constant";
pub const OBJECT_CREATION: &str = "object_creation_expression";
pub const ARRAY_CREATION: &str = "array_creation_expression";
pub const CLASS_BODY: &str = "class_body";
pub const IDENTIFIER: &str = "identifier";
pub const TYPE_IDENTIFIER: &str = "type_identifier";
pub const SCOPED_IDENTIFIER: &str = "scoped_identifier";
pub const SCOPED_TYPE_IDENTIFIER: &str = "scoped_type_identifier";
pub const TYPE_ARGUMENTS: &str = "type_arguments";
pub const FORMAL_PARAMETER: &str = "formal_parameter";
pub const SPREAD_PARAMETER: &str = "spread_parameter";
pub const VARIABLE_DECLARATOR: &str = "variable_declarator";
pub const MARKER_ANNOTATION: &str = "marker_annotation";
pub const ANNOTATION: &str = "annotation";
pub const PUBLIC: &str = "public";
pub const PROTECTED: &str = "protected";
pub const PRIVATE: &str = "private";
pub const STATIC: &str = "static";
pub const FINAL: &str = "final";
pub const ABSTRACT: &str = "abstract";
pub const STRICTFP: &str = "strictfp";
pub const UNNAMED: &str = "_";
pub const NAME_FIELD: &str = "name";
pub const BODY_FIELD: &str = "body";
pub const TYPE_FIELD: &str = "type";
pub const PARAMETERS_FIELD: &str = "parameters";
const NAME_SEPARATOR: &str = ".";
const ASTERISK: &str = "asterisk";
const STAR: &str = "*";

pub fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

pub fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

pub fn modifiers(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == MODIFIERS)
}

pub fn modifier_tokens<'tree>(node: Node<'tree>, keyword: &str) -> Vec<Node<'tree>> {
    modifiers(node).map_or_else(Vec::new, |modifiers| {
        children(modifiers)
            .into_iter()
            .filter(|child| child.kind() == keyword)
            .collect()
    })
}

pub fn has_modifier(node: Node<'_>, keyword: &str) -> bool {
    !modifier_tokens(node, keyword).is_empty()
}

pub fn is_type_declaration(node: Node<'_>) -> bool {
    TYPE_DECLARATIONS.contains(&node.kind())
}

pub fn enclosing_declaration(node: Node<'_>) -> Option<Node<'_>> {
    let parent = node.parent()?;
    let body = if parent.kind() == ENUM_BODY_DECLARATIONS {
        parent.parent()?
    } else {
        parent
    };
    body.parent()
}

pub fn qualified_name(node: Node<'_>, source: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        match current.kind() {
            IDENTIFIER | TYPE_IDENTIFIER => parts.push(text(current, source)),
            ASTERISK => parts.push(STAR),
            TYPE_ARGUMENTS | ANNOTATION | MARKER_ANNOTATION => {}
            _ => pending.extend(named_children(current).into_iter().rev()),
        }
    }
    parts.join(NAME_SEPARATOR)
}

pub fn parameters(declaration: Node<'_>) -> Vec<Node<'_>> {
    declaration
        .child_by_field_name(PARAMETERS_FIELD)
        .map_or_else(Vec::new, named_children)
        .into_iter()
        .filter(|parameter| [FORMAL_PARAMETER, SPREAD_PARAMETER].contains(&parameter.kind()))
        .collect()
}

pub fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    parameter.child_by_field_name(NAME_FIELD).or_else(|| {
        named_children(parameter)
            .into_iter()
            .find(|child| child.kind() == VARIABLE_DECLARATOR)
            .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD))
    })
}

pub fn parameter_type(parameter: Node<'_>) -> Option<Node<'_>> {
    parameter.child_by_field_name(TYPE_FIELD).or_else(|| {
        named_children(parameter)
            .into_iter()
            .find(|child| child.kind() != MODIFIERS)
    })
}
