use tree_sitter::Node;

use super::{
    CLOSE_BRACE, COMMENT_KINDS, Context, OPEN_BRACE, RepeatedBlock, RepeatedKind, Token, all_equivalent,
    content_hash, tokens,
};
use crate::duplicates::image_hash;
use crate::syntax::{named_children, one_based};

fn if_branches(node: Node<'_>) -> (Vec<Node<'_>>, bool) {
    let mut branches: Vec<Node<'_>> = node.child_by_field_name("consequence").into_iter().collect();
    let mut alternative = node.child_by_field_name("alternative");
    while let Some(clause) = alternative {
        let Some(statement) = named_children(clause)
            .into_iter()
            .find(|child| !COMMENT_KINDS.contains(&child.kind()))
        else {
            break;
        };
        if statement.kind() != "if_statement" {
            branches.push(statement);
            return (branches, true);
        }
        branches.extend(statement.child_by_field_name("consequence"));
        alternative = statement.child_by_field_name("alternative");
    }
    (branches, false)
}

pub fn repeated_if_branches(context: &Context<'_>, node: Node<'_>) -> Vec<RepeatedBlock> {
    if node
        .parent()
        .is_some_and(|parent| parent.kind() == "else_clause" || parent.kind() == "if_statement")
    {
        return Vec::new();
    }
    let (branches, ends_with_else) = if_branches(node);
    let groups: Vec<Vec<Node<'_>>> = branches.iter().map(|&branch| vec![branch]).collect();
    repeated_groups(context, &branches, &groups, ends_with_else)
}

fn case_statements(case: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = case.walk();
    let statements: Vec<Node<'_>> = case
        .children_by_field_name("body", &mut cursor)
        .filter(|statement| !COMMENT_KINDS.contains(&statement.kind()))
        .collect();
    let expanded = match statements.as_slice() {
        [single] if single.kind() == "statement_block" => named_children(*single)
            .into_iter()
            .filter(|statement| !COMMENT_KINDS.contains(&statement.kind()))
            .collect(),
        _ => statements,
    };
    match expanded.split_last() {
        Some((last, rest)) if last.kind() == "break_statement" => rest.to_vec(),
        _ => expanded,
    }
}

pub fn repeated_cases(context: &Context<'_>, node: Node<'_>) -> Vec<RepeatedBlock> {
    let cases: Vec<Node<'_>> = node
        .child_by_field_name("body")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|case| case.kind() == "switch_case" || case.kind() == "switch_default")
        .collect();
    let ends_with_default = cases.iter().any(|case| case.kind() == "switch_default");
    let bodies: Vec<Vec<Node<'_>>> = cases.iter().map(|&case| case_statements(case)).collect();
    let (non_empty_cases, non_empty_bodies): (Vec<Node<'_>>, Vec<Vec<Node<'_>>>) = cases
        .iter()
        .copied()
        .zip(bodies.iter().cloned())
        .filter(|(_, body)| !body.is_empty())
        .unzip();
    if all_consecutive_equivalent(&non_empty_bodies, ends_with_default, context.source) {
        return consecutive_repeats(context, &non_empty_cases, &non_empty_bodies);
    }
    pairwise_repeats(context, &cases, &bodies)
}

fn repeated_groups(
    context: &Context<'_>,
    anchors: &[Node<'_>],
    groups: &[Vec<Node<'_>>],
    has_default: bool,
) -> Vec<RepeatedBlock> {
    if all_consecutive_equivalent(groups, has_default, context.source) {
        return consecutive_repeats(context, anchors, groups);
    }
    pairwise_repeats(context, anchors, groups)
}

fn all_consecutive_equivalent(groups: &[Vec<Node<'_>>], has_default: bool, source: &[u8]) -> bool {
    !has_default
        && groups.len() > 1
        && groups
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if all_equivalent(right, left, source)))
}

fn consecutive_repeats(
    context: &Context<'_>,
    anchors: &[Node<'_>],
    groups: &[Vec<Node<'_>>],
) -> Vec<RepeatedBlock> {
    anchors
        .iter()
        .zip(groups)
        .skip(1)
        .zip(anchors)
        .map(|((&anchor, group), &previous)| branch_block(context, anchor, group, previous))
        .collect()
}

fn pairwise_repeats(
    context: &Context<'_>,
    anchors: &[Node<'_>],
    groups: &[Vec<Node<'_>>],
) -> Vec<RepeatedBlock> {
    let mut found = Vec::new();
    for (index, (&anchor, group)) in anchors.iter().zip(groups).enumerate().skip(1) {
        if !has_required_size(group, context.source) {
            continue;
        }
        let original = anchors
            .iter()
            .zip(groups)
            .take(index)
            .find(|(_, earlier)| all_equivalent(group, earlier, context.source));
        if let Some((&previous, _)) = original {
            found.push(branch_block(context, anchor, group, previous));
        }
    }
    found
}

fn has_required_size(group: &[Node<'_>], source: &[u8]) -> bool {
    let (Some(&first), Some(&last)) = (group.first(), group.last()) else {
        return false;
    };
    let open = image_hash(OPEN_BRACE);
    let close = image_hash(CLOSE_BRACE);
    let kept: Vec<Token> = tokens(first, source)
        .into_iter()
        .chain(tokens(last, source))
        .filter(|token| token.image != open && token.image != close)
        .collect();
    match (kept.first(), kept.last()) {
        (Some(start), Some(end)) => end.end_line > start.line,
        _ => false,
    }
}

fn branch_block(
    context: &Context<'_>,
    anchor: Node<'_>,
    group: &[Node<'_>],
    previous: Node<'_>,
) -> RepeatedBlock {
    let line = one_based(anchor.start_position().row);
    let end_line = group
        .last()
        .map_or(line, |last| one_based(last.end_position().row));
    RepeatedBlock {
        kind: RepeatedKind::Branch,
        line,
        end_line,
        original_line: one_based(previous.start_position().row),
        lines: end_line.saturating_sub(line).saturating_add(1),
        content: content_hash(group, context.source),
    }
}
