use tree_sitter::Node;

use super::body_descendants;
use crate::profile::Profile;
use crate::syntax::logical_operator;

const BASE_COMPLEXITY: usize = 1;

pub(super) fn cyclomatic(unit: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    let decisions = body_descendants(unit, profile)
        .into_iter()
        .filter(|&node| is_decision(profile, node, source))
        .count();
    BASE_COMPLEXITY.saturating_add(decisions)
}

fn is_decision(profile: &Profile, node: Node<'_>, source: &[u8]) -> bool {
    let kind = node.kind();
    if profile.decisions.contains(&kind) {
        return true;
    }
    if profile.cases.kinds.contains(&kind) {
        return !is_default_case(profile, node);
    }
    logical_operator(profile, node, source, profile.logical.cyclomatic_operators).is_some()
}

fn is_default_case(profile: &Profile, node: Node<'_>) -> bool {
    profile
        .cases
        .default_marker
        .is_some_and(|marker| node.child(0).is_some_and(|first| first.kind() == marker))
}
