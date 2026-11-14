use tree_sitter::Node;

use super::{CONSTANT, FIELD, INTERFACE_BODIES, RESOURCE, has_safe_varargs};
use crate::lint::java::{
    ABSTRACT, ANNOTATION_TYPE, CLASS, CLASS_BODY, CONSTRUCTOR, ENUM, ENUM_CONSTANT, FINAL, INTERFACE, METHOD,
    NAME_FIELD, OBJECT_CREATION, PRIVATE, PUBLIC, RECORD, STATIC, VARIABLE_DECLARATOR, enclosing_declaration,
    has_modifier, modifier_tokens,
};
use crate::syntax::{descendants, named_children};

const ENUM_BODY: &str = "enum_body";

pub struct Flagged<'tree> {
    pub modifier: Node<'tree>,
    pub location: Node<'tree>,
}

pub fn flagged<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<Flagged<'tree>> {
    descendants(root)
        .into_iter()
        .flat_map(|node| {
            let location = report_location(node);
            unnecessary(node, source)
                .into_iter()
                .map(move |modifier| Flagged { modifier, location })
        })
        .collect()
}

fn report_location(node: Node<'_>) -> Node<'_> {
    let declarator = named_children(node)
        .into_iter()
        .find(|child| child.kind() == VARIABLE_DECLARATOR);
    declarator
        .unwrap_or(node)
        .child_by_field_name(NAME_FIELD)
        .filter(|_| node.kind() != RESOURCE)
        .unwrap_or(node)
}

fn unnecessary<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let in_interface = node
        .parent()
        .is_some_and(|parent| INTERFACE_BODIES.contains(&parent.kind()));
    let nested = enclosing_declaration(node).is_some();
    let keywords: &[&str] = match node.kind() {
        INTERFACE | ANNOTATION_TYPE if in_interface => &[PUBLIC, STATIC, ABSTRACT],
        INTERFACE | ANNOTATION_TYPE if nested => &[STATIC, ABSTRACT],
        INTERFACE | ANNOTATION_TYPE => &[ABSTRACT],
        CLASS | ENUM if in_interface => &[PUBLIC, STATIC],
        ENUM => &[STATIC],
        RECORD => &[STATIC, FINAL],
        FIELD | CONSTANT if in_interface => &[PUBLIC, STATIC, FINAL],
        CONSTRUCTOR if enclosing_declaration(node).is_some_and(|owner| owner.kind() == ENUM) => &[PRIVATE],
        RESOURCE => &[FINAL],
        METHOD => return method_modifiers(node, in_interface, source),
        _ => &[],
    };
    tokens(node, keywords)
}

fn method_modifiers<'tree>(method: Node<'tree>, in_interface: bool, source: &[u8]) -> Vec<Node<'tree>> {
    let mut found = if in_interface {
        tokens(method, &[PUBLIC, ABSTRACT])
    } else {
        Vec::new()
    };
    if !has_safe_varargs(method, source) && (has_modifier(method, PRIVATE) || cannot_be_overridden(method)) {
        found.extend(modifier_tokens(method, FINAL));
    }
    found
}

fn cannot_be_overridden(method: Node<'_>) -> bool {
    enclosing_declaration(method).is_some_and(|owner| match owner.kind() {
        CLASS => has_modifier(owner, FINAL),
        OBJECT_CREATION | ENUM_CONSTANT | RECORD => true,
        ENUM => !has_constant_bodies(owner),
        _ => false,
    })
}

fn has_constant_bodies(declaration: Node<'_>) -> bool {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == ENUM_BODY)
        .flat_map(named_children)
        .filter(|child| child.kind() == ENUM_CONSTANT)
        .any(|constant| {
            named_children(constant)
                .iter()
                .any(|child| child.kind() == CLASS_BODY)
        })
}

fn tokens<'tree>(node: Node<'tree>, keywords: &[&str]) -> Vec<Node<'tree>> {
    keywords
        .iter()
        .flat_map(|keyword| modifier_tokens(node, keyword))
        .collect()
}
