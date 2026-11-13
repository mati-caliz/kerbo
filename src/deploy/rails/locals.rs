use std::ops::Range;

use tree_sitter::Node;

use super::arguments::raw;
use super::loops::{is_assigned, is_interpolated_alone, is_method_name, without_freeze};
use super::rewrite::{Emitter, Expansion, Value, value};
use crate::syntax::{descendants, named_children};

const ASSIGNMENT: &str = "assignment";
const IDENTIFIER: &str = "identifier";
const CONSTANT: &str = "constant";
const CALL: &str = "call";
const INTERPOLATION: &str = "interpolation";
const CONDITIONAL: &str = "conditional";
const QUALIFYING_PARENTS: &[&str] = &["scope_resolution", "class", "module"];
const PARAMETER_LISTS: &[&str] = &["block_parameters", "method_parameters", "lambda_parameters"];
const TEST_ENVIRONMENT_CHECK: &str = "Rails.env.test?";
const NEWLINE: char = '\n';

struct Binding {
    name: String,
    scope: Range<usize>,
    value: Value,
}

struct Replacement {
    range: Range<usize>,
    text: String,
}

pub fn inline(root: Node<'_>, source: &[u8]) -> Option<Expansion> {
    let nodes = descendants(root);
    let bindings: Vec<Binding> = nodes
        .iter()
        .filter_map(|node| binding(*node, root, &nodes, source))
        .collect();
    let mut replacements: Vec<Replacement> = nodes
        .iter()
        .filter_map(|node| replacement(*node, &bindings, source))
        .collect();
    replacements.sort_by_key(|replacement| replacement.range.start);
    let mut emitter = Emitter::new(source);
    let mut position = 0;
    for replacement in replacements {
        if replacement.range.start < position {
            continue;
        }
        emitter.original(position..replacement.range.start);
        let row = emitter.row_of(replacement.range.start);
        emitter.inserted(&replacement.text, row);
        position = replacement.range.end;
    }
    if position == 0 {
        return None;
    }
    emitter.original(position..source.len());
    Some(emitter.finish())
}

fn binding(node: Node<'_>, root: Node<'_>, nodes: &[Node<'_>], source: &[u8]) -> Option<Binding> {
    if node.kind() != ASSIGNMENT {
        return None;
    }
    let left = node.child_by_field_name("left")?;
    let value = value(without_freeze(node.child_by_field_name("right")?, source), source)?;
    let name = raw(left, source);
    let scope = match left.kind() {
        CONSTANT if assignments_of(&name, nodes, source) == 1 => root.start_byte()..root.end_byte(),
        IDENTIFIER => local_scope(node, &name, source)?,
        _ => return None,
    };
    let rebound = nodes.iter().any(|other| {
        other.start_byte() >= scope.start
            && other.end_byte() <= scope.end
            && *other != left
            && raw(*other, source) == name
            && (is_assigned(*other) || is_parameter(*other))
    });
    (!rebound).then_some(Binding { name, scope, value })
}

fn assignments_of(name: &str, nodes: &[Node<'_>], source: &[u8]) -> usize {
    nodes
        .iter()
        .filter(|node| node.kind() == CONSTANT && is_assigned(**node) && raw(**node, source) == name)
        .count()
}

fn local_scope(assignment: Node<'_>, name: &str, source: &[u8]) -> Option<Range<usize>> {
    let parent = assignment.parent()?;
    let mut sibling = assignment.next_named_sibling();
    while let Some(statement) = sibling {
        let reassigns = statement.kind() == ASSIGNMENT
            && statement
                .child_by_field_name("left")
                .is_some_and(|left| raw(left, source) == name);
        if reassigns {
            return Some(assignment.end_byte()..statement.start_byte());
        }
        sibling = statement.next_named_sibling();
    }
    Some(assignment.end_byte()..parent.end_byte())
}

fn is_parameter(node: Node<'_>) -> bool {
    node.parent()
        .is_some_and(|parent| PARAMETER_LISTS.contains(&parent.kind()))
}

fn replacement(node: Node<'_>, bindings: &[Binding], source: &[u8]) -> Option<Replacement> {
    let range = node.start_byte()..node.end_byte();
    let text = match node.kind() {
        INTERPOLATION => match named_children(node).as_slice() {
            [inner] if inner.kind() == CONDITIONAL => {
                value(test_environment_branch(*inner, source)?, source)?.text
            }
            [inner] => bound(*inner, bindings, source)?.text.clone(),
            _ => return None,
        },
        CONDITIONAL if !is_interpolated_alone(node) => raw(test_environment_branch(node, source)?, source),
        IDENTIFIER | CONSTANT if is_reference(node) => bound(node, bindings, source)?.literal.clone(),
        _ => return None,
    };
    (!text.contains(NEWLINE)).then_some(Replacement { range, text })
}

fn test_environment_branch<'tree>(conditional: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let condition = conditional.child_by_field_name("condition")?;
    (raw(condition, source) == TEST_ENVIRONMENT_CHECK)
        .then(|| conditional.child_by_field_name("alternative"))
        .flatten()
}

fn is_reference(node: Node<'_>) -> bool {
    !is_interpolated_alone(node)
        && !is_method_name(node)
        && !is_assigned(node)
        && !is_parameter(node)
        && node.parent().is_none_or(|parent| {
            !(QUALIFYING_PARENTS.contains(&parent.kind())
                || (parent.kind() == CALL && parent.child_by_field_name("method") == Some(node)))
        })
}

fn bound<'binding>(node: Node<'_>, bindings: &'binding [Binding], source: &[u8]) -> Option<&'binding Value> {
    if ![IDENTIFIER, CONSTANT].contains(&node.kind()) {
        return None;
    }
    let name = raw(node, source);
    bindings
        .iter()
        .filter(|binding| binding.name == name && binding.scope.contains(&node.start_byte()))
        .max_by_key(|binding| binding.scope.start)
        .map(|binding| &binding.value)
}
