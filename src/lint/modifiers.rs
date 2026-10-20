use tree_sitter::Node;

use super::java::{
    ABSTRACT, ANNOTATION_TYPE, ARRAY_CREATION, BODY_FIELD, CLASS, CONSTRUCTOR, ENUM, ENUM_CONSTANT, FINAL,
    INTERFACE, MARKER_ANNOTATION, METHOD, NAME_FIELD, OBJECT_CREATION, PRIVATE, PROGRAM, PROTECTED, PUBLIC,
    RECORD, STATIC, STRICTFP, UNNAMED, VARIABLE_DECLARATOR, children, enclosing_declaration, has_modifier,
    modifier_tokens, modifiers, parameters, text,
};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::syntax::{descendants, named_children};

const FIELD: &str = "field_declaration";
const CONSTANT: &str = "constant_declaration";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const FOR_EACH: &str = "enhanced_for_statement";
const ANNOTATION_ELEMENT: &str = "annotation_type_element_declaration";
const RESOURCE: &str = "resource";
const CATCH: &str = "catch_clause";
const LAMBDA: &str = "lambda_expression";
const TYPE_PATTERN: &str = "type_pattern";
const VISITED: [&str; 16] = [
    METHOD,
    FIELD,
    CONSTANT,
    LOCAL_VARIABLE,
    FOR_EACH,
    ANNOTATION_ELEMENT,
    INTERFACE,
    CONSTRUCTOR,
    CLASS,
    ENUM,
    RESOURCE,
    ANNOTATION_TYPE,
    RECORD,
    CATCH,
    LAMBDA,
    TYPE_PATTERN,
];
const VARIABLES: [&str; 3] = [FIELD, CONSTANT, LOCAL_VARIABLE];
const INTERFACE_BODIES: [&str; 2] = ["interface_body", "annotation_type_body"];
const CATCH_PARAMETER: &str = "catch_formal_parameter";
const SAFE_VARARGS: &str = "SafeVarargs";

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut flagged: Vec<Node<'_>> = descendants(root)
        .into_iter()
        .filter(|node| VISITED.contains(&node.kind()))
        .flat_map(|node| redundant_modifiers(node, source))
        .collect();
    flagged.sort_by_key(Node::start_byte);
    flagged.dedup_by_key(|modifier| modifier.start_byte());
    flagged
        .into_iter()
        .map(|modifier| {
            finding(
                LintKind::RedundantModifier,
                modifier,
                text(modifier, source),
                Some(Fix::remove(modifier, source)),
            )
        })
        .collect()
}

fn redundant_modifiers<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let mut found = by_declaration(node, source);
    if node
        .parent()
        .is_some_and(|parent| INTERFACE_BODIES.contains(&parent.kind()))
    {
        found.extend(implicit_in_interface(node));
    }
    found.extend(modifier_tokens(node, STRICTFP));
    found
}

fn by_declaration<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    match node.kind() {
        INTERFACE | ANNOTATION_TYPE => [STATIC, ABSTRACT]
            .into_iter()
            .filter_map(|keyword| modifier_tokens(node, keyword).into_iter().next())
            .collect(),
        ENUM => modifier_tokens(node, STATIC),
        CONSTRUCTOR => constructor_modifiers(node),
        METHOD => method_modifiers(node, source),
        RESOURCE => modifier_tokens(node, FINAL),
        RECORD => [FINAL, STATIC]
            .into_iter()
            .flat_map(|keyword| modifier_tokens(node, keyword))
            .collect(),
        kind if VARIABLES.contains(&kind) => unnamed_final(node, &declarator_names(node), source),
        FOR_EACH | TYPE_PATTERN => unnamed_final(node, &own_name(node), source),
        CATCH => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == CATCH_PARAMETER)
            .flat_map(|parameter| unnamed_final(parameter, &own_name(parameter), source))
            .collect(),
        LAMBDA => parameters(node)
            .into_iter()
            .flat_map(|parameter| unnamed_final(parameter, &own_name(parameter), source))
            .collect(),
        _ => Vec::new(),
    }
}

fn declarator_names(node: Node<'_>) -> Vec<Node<'_>> {
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
        .collect()
}

fn own_name(node: Node<'_>) -> Vec<Node<'_>> {
    node.child_by_field_name(NAME_FIELD).into_iter().collect()
}

fn unnamed_final<'tree>(node: Node<'tree>, names: &[Node<'_>], source: &[u8]) -> Vec<Node<'tree>> {
    if names.iter().any(|name| text(*name, source) == UNNAMED) {
        modifier_tokens(node, FINAL)
    } else {
        Vec::new()
    }
}

fn implicit_in_interface(node: Node<'_>) -> Vec<Node<'_>> {
    let kind = node.kind();
    keywords(node)
        .into_iter()
        .filter(|modifier| match modifier.kind() {
            PUBLIC => true,
            STATIC => kind != METHOD,
            ABSTRACT | FINAL => kind != CLASS,
            _ => false,
        })
        .collect()
}

fn keywords(node: Node<'_>) -> Vec<Node<'_>> {
    modifiers(node).map_or_else(Vec::new, |modifiers| {
        children(modifiers)
            .into_iter()
            .filter(|child| !child.is_named())
            .collect()
    })
}

fn constructor_modifiers(constructor: Node<'_>) -> Vec<Node<'_>> {
    let Some(owner) = enclosing_declaration(constructor) else {
        return Vec::new();
    };
    if owner.kind() == ENUM {
        return keywords(constructor).into_iter().take(1).collect();
    }
    if is_public_type(owner) || has_modifier(owner, PROTECTED) {
        Vec::new()
    } else {
        modifier_tokens(constructor, PUBLIC)
    }
}

fn is_public_type(declaration: Node<'_>) -> bool {
    let is_public = has_modifier(declaration, PUBLIC);
    if declaration
        .parent()
        .is_some_and(|parent| parent.kind() == PROGRAM)
    {
        return is_public;
    }
    enclosing_declaration(declaration)
        .filter(|outer| is_public || outer.kind() == INTERFACE)
        .is_some_and(is_public_type)
}

fn method_modifiers<'tree>(method: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let final_is_implied = has_modifier(method, PRIVATE) || owner_implies_final(method);
    if final_is_implied && !has_safe_varargs(method, source) {
        found.extend(modifier_tokens(method, FINAL));
    }
    if method.child_by_field_name(BODY_FIELD).is_none() {
        found.extend(
            parameters(method)
                .into_iter()
                .flat_map(|parameter| modifier_tokens(parameter, FINAL)),
        );
    }
    found
}

fn owner_implies_final(method: Node<'_>) -> bool {
    let mut ancestor = method.parent();
    while let Some(node) = ancestor {
        match node.kind() {
            CLASS => return has_modifier(node, FINAL),
            OBJECT_CREATION | ARRAY_CREATION | ENUM_CONSTANT => return true,
            ENUM => return has_modifier(method, STATIC),
            _ => ancestor = node.parent(),
        }
    }
    false
}

fn has_safe_varargs(method: Node<'_>, source: &[u8]) -> bool {
    modifiers(method).is_some_and(|modifiers| {
        named_children(modifiers).into_iter().any(|annotation| {
            annotation.kind() == MARKER_ANNOTATION
                && annotation
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == SAFE_VARARGS)
        })
    })
}
