use tree_sitter::Node;

use super::{body_descendants, is_separate_unit};
use crate::profile::Profile;
use crate::syntax::{logical_operator, named_children};

const BASE_COMPLEXITY: usize = 1;
const CASE_PATTERN: &str = "case_pattern";
const GUARD_FIELD: &str = "guard";
const UNION_PATTERN: &str = "union_pattern";
const AS_PATTERN: &str = "as_pattern";
const DOTTED_NAME: &str = "dotted_name";

pub(super) fn cyclomatic(unit: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    BASE_COMPLEXITY.saturating_add(decisions_below(unit, profile, source))
}

pub(super) fn cyclomatic_including_root(root: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    if is_separate_unit(root, profile) {
        return BASE_COMPLEXITY;
    }
    let root_decision = usize::from(is_decision(profile, root, source));
    cyclomatic(root, profile, source).saturating_add(root_decision)
}

fn decisions_below(unit: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    let descendants = if profile.nested_units_add_to_cyclomatic {
        all_descendants(unit)
    } else {
        body_descendants(unit, profile)
    };
    descendants
        .into_iter()
        .filter(|&node| is_decision(profile, node, source))
        .count()
}

fn all_descendants(unit: Node<'_>) -> Vec<Node<'_>> {
    let mut descendants = Vec::new();
    let mut cursor = unit.walk();
    let mut pending: Vec<Node<'_>> = unit.children(&mut cursor).collect();
    while let Some(node) = pending.pop() {
        descendants.push(node);
        let mut child_cursor = node.walk();
        pending.extend(node.children(&mut child_cursor));
    }
    descendants
}

fn is_decision(profile: &Profile, node: Node<'_>, source: &[u8]) -> bool {
    let kind = node.kind();
    if profile.decisions.contains(&kind) {
        return true;
    }
    if profile.nested_units_add_to_cyclomatic && profile.units.contains(&kind) {
        return true;
    }
    if profile.cases.kinds.contains(&kind) {
        return !is_default_case(profile, node, source);
    }
    if profile
        .decision_parents
        .iter()
        .any(|&(decision_kind, parent_kind)| {
            decision_kind == kind && node.parent().is_some_and(|parent| parent.kind() == parent_kind)
        })
    {
        return true;
    }
    if profile
        .decision_fields
        .iter()
        .any(|&(decision_kind, field)| decision_kind == kind && node.child_by_field_name(field).is_some())
    {
        return true;
    }
    logical_operator(profile, node, source, profile.logical.cyclomatic_operators).is_some()
}

fn is_default_case(profile: &Profile, node: Node<'_>, source: &[u8]) -> bool {
    let marked = profile
        .cases
        .default_marker
        .is_some_and(|marker| node.child(0).is_some_and(|first| first.kind() == marker));
    let wildcard = profile
        .cases
        .wildcard_pattern
        .is_some_and(|wildcard| is_irrefutable_last_case(node, source, wildcard));
    marked || wildcard
}

// Como ruff (C901): sólo el último `case`, sin guarda y con un patrón que siempre coincide
// (`_`, un nombre suelto, `_ as x` o una alternativa que incluya uno de esos) cuenta como `else`.
fn is_irrefutable_last_case(node: Node<'_>, source: &[u8], wildcard: &str) -> bool {
    let is_last = node
        .next_named_sibling()
        .is_none_or(|next| next.kind() != node.kind());
    let patterns: Vec<Node<'_>> = named_children(node)
        .into_iter()
        .filter(|child| child.kind() == CASE_PATTERN)
        .collect();
    is_last
        && node.child_by_field_name(GUARD_FIELD).is_none()
        && matches!(patterns.as_slice(), [single] if is_irrefutable(*single, source, wildcard))
}

fn is_irrefutable(pattern: Node<'_>, source: &[u8], wildcard: &str) -> bool {
    let children = named_children(pattern);
    match pattern.kind() {
        UNION_PATTERN => (0..pattern.child_count())
            .filter_map(|index| pattern.child(index))
            .any(|child| {
                if child.is_named() {
                    is_irrefutable(child, source, wildcard)
                } else {
                    child.utf8_text(source).ok() == Some(wildcard)
                }
            }),
        AS_PATTERN => children
            .first()
            .is_some_and(|&inner| is_irrefutable(inner, source, wildcard)),
        DOTTED_NAME => children.len() == 1,
        _ => match children.as_slice() {
            [] => pattern.utf8_text(source).ok() == Some(wildcard),
            [single] => is_irrefutable(*single, source, wildcard),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests;
