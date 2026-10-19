use tree_sitter::Node;

use super::{body_descendants, is_separate_unit};
use crate::profile::Profile;
use crate::syntax::logical_operator;

const BASE_COMPLEXITY: usize = 1;

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
    let wildcard = profile.cases.wildcard_pattern.is_some_and(|wildcard| {
        node.named_child(0)
            .and_then(|pattern| pattern.utf8_text(source).ok())
            .is_some_and(|pattern| pattern == wildcard)
    });
    marked || wildcard
}
