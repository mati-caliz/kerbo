use tree_sitter::Node;

use crate::lint::ecmascript::nodes::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, COMMENT, PARENTHESIZED, is_function, parent_without_parentheses,
    without_parentheses,
};
use crate::lint::ecmascript::scope::{DefinitionKind, Reference, ScopeTree, Variable};
use crate::syntax::named_children;

const LOOPS: &[&str] = &[
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
];
const LOGICAL_ASSIGNMENTS: &[&str] = &["&&=", "||=", "??="];
const FUNCTION_VALUES: &[&str] = &[
    "function_expression",
    "function",
    "generator_function",
    "arrow_function",
];
const TYPE_PREDICATE: &str = "type_predicate";
const TYPE_CONTAINERS: &[&str] = &["type_query", "asserts"];
const CHAIN_LINKS: &[&str] = &[
    "member_expression",
    "nested_identifier",
    "instantiation_expression",
];
const TRANSPARENT_PARENTS: &[&str] = &[PARENTHESIZED, "arguments"];
const STATEMENT_LIKE: &[&str] = &["statement_block", "program", "class_body"];

pub fn is_used(tree: &ScopeTree<'_>, variable: &Variable<'_>, source: &[u8]) -> bool {
    let functions = function_definitions(variable);
    let types: Vec<Node<'_>> = definition_nodes(variable, |kind, node| {
        kind == DefinitionKind::Type
            && ["interface_declaration", "type_alias_declaration"].contains(&node.kind())
    });
    let modules = definition_nodes(variable, |kind, _| kind == DefinitionKind::ModuleName);
    let enums = definition_nodes(variable, |kind, _| kind == DefinitionKind::EnumName);
    let imported_as_type = variable
        .definitions
        .iter()
        .all(|definition| matches!(definition.kind, DefinitionKind::ImportBinding { type_only: true }));
    let mut right_hand: Option<Node<'_>> = None;
    tree.references_of(variable).any(|reference| {
        let for_itself = is_read_for_itself(reference, right_hand, source);
        right_hand = right_hand_node(tree, variable, reference, right_hand);
        reference.is_read()
            && !for_itself
            && (imported_as_type || !is_type_only(variable, reference))
            && !tree.is_inside_block(reference, &functions)
            && !types.iter().any(|node| is_inside(reference.identifier, *node))
            && !tree.is_inside_block(reference, &modules)
            && !tree.is_inside_block(reference, &enums)
    })
}

fn definition_nodes<'tree>(
    variable: &Variable<'tree>,
    wanted: impl Fn(DefinitionKind, Node<'tree>) -> bool,
) -> Vec<Node<'tree>> {
    variable
        .definitions
        .iter()
        .filter(|definition| wanted(definition.kind, definition.node))
        .map(|definition| definition.node)
        .collect()
}

fn function_definitions<'tree>(variable: &Variable<'tree>) -> Vec<Node<'tree>> {
    variable
        .definitions
        .iter()
        .filter_map(|definition| match definition.kind {
            DefinitionKind::FunctionName => Some(definition.node),
            DefinitionKind::Variable => definition
                .node
                .child_by_field_name("value")
                .map(without_parentheses)
                .filter(|value| FUNCTION_VALUES.contains(&value.kind())),
            DefinitionKind::CatchClause
            | DefinitionKind::ClassName
            | DefinitionKind::ImportBinding { .. }
            | DefinitionKind::Parameter
            | DefinitionKind::EnumMember
            | DefinitionKind::EnumName
            | DefinitionKind::ModuleName
            | DefinitionKind::Type => None,
        })
        .collect()
}

pub fn is_type_only(variable: &Variable<'_>, reference: &Reference<'_>) -> bool {
    if is_in_type_container(reference.identifier) {
        return true;
    }
    variable
        .definitions
        .iter()
        .any(|definition| definition.kind == DefinitionKind::Variable)
        && !reference.is_value()
}

fn is_in_type_container(identifier: Node<'_>) -> bool {
    let mut current = identifier;
    while let Some(parent) = current.parent() {
        if parent.kind() == TYPE_PREDICATE {
            return parent
                .child_by_field_name("name")
                .is_some_and(|name| name.id() == current.id());
        }
        if TYPE_CONTAINERS.contains(&parent.kind()) {
            return true;
        }
        if !CHAIN_LINKS.contains(&parent.kind()) {
            return false;
        }
        current = parent;
    }
    false
}

pub fn is_inside(inner: Node<'_>, outer: Node<'_>) -> bool {
    inner.start_byte() >= outer.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn is_unused_expression(node: Node<'_>) -> bool {
    let Some(parent) = parent_without_parentheses(node) else {
        return false;
    };
    match parent.kind() {
        "expression_statement" => true,
        "sequence_expression" => {
            let last = named_children(parent)
                .into_iter()
                .rev()
                .find(|child| child.kind() != COMMENT)
                .map(without_parentheses);
            last.is_none_or(|last| last.id() != node.id()) || is_unused_expression(parent)
        }
        _ => false,
    }
}

fn is_in_loop(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if is_function(candidate) {
            return false;
        }
        if LOOPS.contains(&candidate.kind()) {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn assignment_parent(identifier: Node<'_>) -> Option<Node<'_>> {
    parent_without_parentheses(identifier)
        .filter(|parent| [ASSIGNMENT, AUGMENTED_ASSIGNMENT].contains(&parent.kind()))
        .filter(|parent| {
            parent
                .child_by_field_name("left")
                .map(without_parentheses)
                .is_some_and(|left| left.id() == identifier.id())
        })
}

fn right_hand_node<'tree>(
    tree: &ScopeTree<'tree>,
    variable: &Variable<'tree>,
    reference: &Reference<'tree>,
    previous: Option<Node<'tree>>,
) -> Option<Node<'tree>> {
    let identifier = reference.identifier;
    if let Some(previous) = previous.filter(|previous| is_inside(identifier, *previous)) {
        return Some(previous);
    }
    let can_be_used_later = tree.variable_scope_of(reference.from) != tree.variable_scope_of(variable.scope)
        || is_in_loop(identifier);
    let assignment = assignment_parent(identifier).filter(|assignment| is_unused_expression(*assignment))?;
    if can_be_used_later {
        return None;
    }
    assignment.child_by_field_name("right").map(without_parentheses)
}

fn is_read_for_itself(reference: &Reference<'_>, right_hand: Option<Node<'_>>, source: &[u8]) -> bool {
    if !reference.is_read() {
        return false;
    }
    let identifier = reference.identifier;
    let self_assignment = assignment_parent(identifier).is_some_and(|assignment| {
        let operator = assignment
            .child_by_field_name("operator")
            .and_then(|operator| operator.utf8_text(source).ok())
            .unwrap_or_default();
        !LOGICAL_ASSIGNMENTS.contains(&operator) && is_unused_expression(assignment)
    });
    let self_update = parent_without_parentheses(identifier)
        .filter(|parent| parent.kind() == "update_expression")
        .is_some_and(is_unused_expression);
    let inside_right_hand = right_hand.is_some_and(|right_hand| {
        is_inside(identifier, right_hand) && !is_inside_storable_function(identifier, right_hand)
    });
    self_assignment || self_update || inside_right_hand
}

fn is_inside_storable_function(identifier: Node<'_>, right_hand: Node<'_>) -> bool {
    let mut function = identifier.parent();
    while let Some(candidate) = function.filter(|candidate| !is_function(*candidate)) {
        function = candidate.parent();
    }
    function.is_some_and(|function| is_inside(function, right_hand) && is_storable(function, right_hand))
}

fn is_storable(function: Node<'_>, right_hand: Node<'_>) -> bool {
    let mut node = function;
    let mut parent = function.parent();
    while let Some(current) = parent.filter(|current| is_inside(*current, right_hand)) {
        if TRANSPARENT_PARENTS.contains(&current.kind()) {
            parent = current.parent();
            continue;
        }
        match current.kind() {
            "sequence_expression" => {
                let last = named_children(current)
                    .into_iter()
                    .rev()
                    .find(|child| child.kind() != COMMENT);
                if last.is_none_or(|last| without_parentheses(last).id() != node.id()) {
                    return false;
                }
            }
            "call_expression" | "new_expression" => {
                let callee = current
                    .child_by_field_name("function")
                    .or_else(|| current.child_by_field_name("constructor"))
                    .map(without_parentheses);
                return callee.is_none_or(|callee| callee.id() != node.id());
            }
            "assignment_expression" | "augmented_assignment_expression" | "yield_expression" => return true,
            kind if kind.ends_with("_statement")
                || kind.ends_with("_declaration")
                || STATEMENT_LIKE.contains(&kind) =>
            {
                return true;
            }
            _ => {}
        }
        node = current;
        parent = current.parent();
    }
    false
}
