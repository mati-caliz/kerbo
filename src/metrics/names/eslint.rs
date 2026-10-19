use std::collections::HashSet;

use tree_sitter::Node;

use super::{ShortName, field, has_token, named_children, short_name, text};

const MIN_LENGTH: usize = 2;
const EXCEPTIONS: &[&str] = &["i", "j", "_"];
const PRIVATE_PREFIX: char = '#';
const IDENTIFIER: &str = "identifier";
const NAMED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "function",
    "generator_function",
];
const ARROW: &str = "arrow_function";
const METHOD: &str = "method_definition";
const CLASS_OVERLOAD: &str = "method_signature";
const CLASSES: &[&str] = &["class_declaration", "abstract_class_declaration"];
const CLASS_BODY: &str = "class_body";
const OBJECT: &str = "object";
const CLASS_FIELD: &str = "public_field_definition";
const ABSTRACT: &str = "abstract";
const COMPUTED_KEY: &str = "computed_property_name";
const MEMBER_NAMES: &[&str] = &["property_identifier", "private_property_identifier"];
const HERITAGE: &str = "class_heritage";
const EXTENDS: &str = "extends_clause";
const TYPED_PARAMETERS: &[&str] = &["required_parameter", "optional_parameter"];
const PARAMETER_PROPERTY_MODIFIERS: &[&str] = &["accessibility_modifier", "readonly", "override_modifier"];
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const FOR_IN: &str = "for_in_statement";
const CATCH: &str = "catch_clause";
const ARRAY_PATTERN: &str = "array_pattern";
const PAIR_PATTERN: &str = "pair_pattern";
const DEFAULTED: &[&str] = &["assignment_pattern", "object_assignment_pattern"];
const DEFAULTED_NAMES: &[&str] = &[IDENTIFIER, "shorthand_property_identifier_pattern"];
const REST: &str = "rest_pattern";
const IMPORT_CLAUSE: &str = "import_clause";
const NAMESPACE_IMPORT: &str = "namespace_import";
const IMPORT_SPECIFIER: &str = "import_specifier";

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<ShortName> {
    let mut seen = HashSet::new();
    nodes
        .iter()
        .flat_map(|&node| checked_names(node, source))
        .filter(|name| seen.insert(name.id()))
        .filter_map(|name| {
            let raw = text(name, source);
            let bare = raw.strip_prefix(PRIVATE_PREFIX).unwrap_or(raw);
            (bare.chars().count() < MIN_LENGTH && !EXCEPTIONS.contains(&bare)).then(|| short_name(name, bare))
        })
        .collect()
}

fn checked_names<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let kind = node.kind();
    let identifier = |child: Option<Node<'tree>>| child.filter(|child| child.kind() == IDENTIFIER);
    match kind {
        VARIABLE_DECLARATOR => identifier(field(node, "name")).into_iter().collect(),
        kind if NAMED_FUNCTIONS.contains(&kind) => identifier(field(node, "name"))
            .into_iter()
            .chain(parameters(node))
            .collect(),
        ARROW => identifier(field(node, "parameter"))
            .into_iter()
            .chain(identifier(field(node, "body")))
            .chain(parameters(node))
            .collect(),
        METHOD => {
            let in_object = node.parent().is_some_and(|parent| parent.kind() == OBJECT);
            let name = if in_object { None } else { member_name(node) };
            name.into_iter().chain(parameters(node)).collect()
        }
        CLASS_OVERLOAD if node.parent().is_some_and(|parent| parent.kind() == CLASS_BODY) => {
            member_name(node).into_iter().collect()
        }
        kind if CLASSES.contains(&kind) => field(node, "name").into_iter().chain(superclass(node)).collect(),
        CLASS_FIELD if !has_token(node, ABSTRACT) => member_name(node)
            .into_iter()
            .chain(identifier(field(node, "value")))
            .collect(),
        FOR_IN if field(node, "kind").is_some() => identifier(field(node, "left")).into_iter().collect(),
        CATCH => identifier(field(node, "parameter")).into_iter().collect(),
        ARRAY_PATTERN => named_children(node)
            .into_iter()
            .filter(|element| element.kind() == IDENTIFIER)
            .collect(),
        PAIR_PATTERN => {
            let key = field(node, "key").map(|key| text(key, source));
            identifier(field(node, "value"))
                .filter(|value| Some(text(*value, source)) != key)
                .into_iter()
                .collect()
        }
        kind if DEFAULTED.contains(&kind) => field(node, "left")
            .filter(|left| DEFAULTED_NAMES.contains(&left.kind()))
            .into_iter()
            .collect(),
        REST => named_children(node)
            .into_iter()
            .filter(|argument| argument.kind() == IDENTIFIER)
            .collect(),
        IMPORT_CLAUSE | NAMESPACE_IMPORT => named_children(node)
            .into_iter()
            .filter(|imported| imported.kind() == IDENTIFIER)
            .collect(),
        IMPORT_SPECIFIER => {
            let imported = field(node, "name").map(|name| text(name, source));
            field(node, "alias")
                .filter(|alias| Some(text(*alias, source)) != imported)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn member_name(node: Node<'_>) -> Option<Node<'_>> {
    let name = field(node, "name")?;
    if MEMBER_NAMES.contains(&name.kind()) {
        return Some(name);
    }
    (name.kind() == COMPUTED_KEY)
        .then(|| {
            named_children(name)
                .into_iter()
                .find(|key| key.kind() == IDENTIFIER)
        })
        .flatten()
}

fn superclass(class: Node<'_>) -> Option<Node<'_>> {
    named_children(class)
        .into_iter()
        .find(|child| child.kind() == HERITAGE)
        .and_then(|heritage| {
            named_children(heritage)
                .into_iter()
                .find(|clause| clause.kind() == EXTENDS)
        })
        .and_then(|clause| field(clause, "value"))
        .filter(|value| value.kind() == IDENTIFIER)
}

fn parameters(function: Node<'_>) -> Vec<Node<'_>> {
    field(function, "parameters")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|parameter| match parameter.kind() {
            IDENTIFIER => Some(parameter),
            kind if TYPED_PARAMETERS.contains(&kind) => {
                let is_property = named_children(parameter)
                    .iter()
                    .any(|child| PARAMETER_PROPERTY_MODIFIERS.contains(&child.kind()))
                    || has_token(parameter, "readonly");
                field(parameter, "pattern").filter(|pattern| !is_property && pattern.kind() == IDENTIFIER)
            }
            _ => None,
        })
        .collect()
}
