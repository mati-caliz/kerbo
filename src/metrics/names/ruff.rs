use tree_sitter::Node;

use super::{ShortName, field, named_children, short_name, text};

const AMBIGUOUS: &[&str] = &["l", "O", "I"];
const IDENTIFIER: &str = "identifier";
const TARGET_PATTERNS: &[&str] = &[
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "list_splat_pattern",
    "tuple",
    "list",
    "parenthesized_expression",
];
const ASSIGNMENTS: &[&str] = &["assignment", "augmented_assignment"];
const LOOPS: &[&str] = &["for_statement", "for_in_clause"];
const AS_PATTERN: &str = "as_pattern";
const AS_HOLDERS: &[&str] = &["with_item", "except_clause"];
const PARAMETER_LISTS: &[&str] = &["parameters", "lambda_parameters"];
const NAMED_PARAMETERS: &[&str] = &["default_parameter", "typed_default_parameter"];
const UNNAMED_PARAMETERS: &[&str] = &[
    "typed_parameter",
    "list_splat_pattern",
    "dictionary_splat_pattern",
];
const SCOPE_STATEMENTS: &[&str] = &["global_statement", "nonlocal_statement"];
const WALRUS: &str = "named_expression";
const TYPE_ALIAS: &str = "type_alias_statement";
const DEFINITIONS: &[&str] = &["function_definition", "class_definition"];

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<ShortName> {
    nodes
        .iter()
        .flat_map(|&node| bound_names(node))
        .filter_map(|name| {
            let name_text = text(name, source);
            AMBIGUOUS
                .contains(&name_text)
                .then(|| short_name(name, name_text))
        })
        .collect()
}

fn bound_names(node: Node<'_>) -> Vec<Node<'_>> {
    let kind = node.kind();
    if ASSIGNMENTS.contains(&kind) || LOOPS.contains(&kind) {
        return field(node, "left").map(target_names).unwrap_or_default();
    }
    if kind == AS_PATTERN
        && node
            .parent()
            .is_some_and(|parent| AS_HOLDERS.contains(&parent.kind()))
    {
        return field(node, "alias")
            .and_then(|alias| named_children(alias).into_iter().next())
            .map(target_names)
            .unwrap_or_default();
    }
    if PARAMETER_LISTS.contains(&kind) {
        return named_children(node)
            .into_iter()
            .flat_map(parameter_names)
            .collect();
    }
    if SCOPE_STATEMENTS.contains(&kind) {
        return named_children(node)
            .into_iter()
            .filter(|name| name.kind() == IDENTIFIER)
            .collect();
    }
    let named = match kind {
        WALRUS => field(node, "name"),
        TYPE_ALIAS => field(node, "left").and_then(|left| named_children(left).into_iter().next()),
        kind if DEFINITIONS.contains(&kind) => field(node, "name"),
        _ => None,
    };
    named
        .filter(|name| name.kind() == IDENTIFIER)
        .into_iter()
        .collect()
}

fn target_names(target: Node<'_>) -> Vec<Node<'_>> {
    if target.kind() == IDENTIFIER {
        return vec![target];
    }
    if TARGET_PATTERNS.contains(&target.kind()) {
        return named_children(target)
            .into_iter()
            .flat_map(target_names)
            .collect();
    }
    Vec::new()
}

fn parameter_names(parameter: Node<'_>) -> Vec<Node<'_>> {
    let kind = parameter.kind();
    if kind == IDENTIFIER {
        return vec![parameter];
    }
    if NAMED_PARAMETERS.contains(&kind) {
        return field(parameter, "name")
            .filter(|name| name.kind() == IDENTIFIER)
            .into_iter()
            .collect();
    }
    if UNNAMED_PARAMETERS.contains(&kind) {
        return named_children(parameter)
            .into_iter()
            .next()
            .map(parameter_names)
            .unwrap_or_default();
    }
    Vec::new()
}
