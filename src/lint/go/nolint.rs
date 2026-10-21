use tree_sitter::{Node, Parser};

use crate::syntax::one_based;

const COMMENT: &str = "comment";
const PACKAGE_CLAUSE: &str = "package_clause";
const WRAPPERS_WITHOUT_GO_NODE: [&str; 3] = ["statement_list", "expression_list", "argument_list"];
const DIRECTIVE: &str = "nolint";
const LINTER_LIST: &str = "nolint:";
const EVERY_LINTER: &str = "all";
const LEADING_MARKS: &[char] = &['/', ' '];
const DIRECTIVE_ENDINGS: [char; 1] = [' '];
const REASON_SEPARATOR: &str = "//";
const LINTER_SEPARATOR: char = ',';
const TRAILING_GROUP_GAP: usize = 0;
const LEADING_GROUP_GAP: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NolintRange {
    pub line: usize,
    pub last_line: usize,
    pub linters: Vec<String>,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
struct Leaf {
    start_row: usize,
    end_row: usize,
    start_column: usize,
    comment: bool,
}

pub fn ranges(source: &str) -> Vec<NolintRange> {
    if !source.contains(DIRECTIVE) {
        return Vec::new();
    }
    let mut parser = Parser::new();
    if parser.set_language(&tree_sitter_go::LANGUAGE.into()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let root = tree.root_node();
    let mut leaves = Vec::new();
    collect_leaves(root, &mut leaves);
    let mut found = Vec::new();
    for group in comment_groups(&leaves) {
        for comment in &group.comments {
            if let Some(mut range) = parse_directive(&comment_text(root, source, comment)) {
                range.line = one_based(group.start_row);
                found.push(expanded(root, &group, range));
            }
        }
    }
    found
}

#[derive(Debug)]
struct Group {
    comments: Vec<Leaf>,
    start_row: usize,
    end_row: usize,
    start_column: usize,
}

fn collect_leaves(node: Node<'_>, leaves: &mut Vec<Leaf>) {
    if node.child_count() == 0 || node.kind() == COMMENT {
        if node.end_byte() > node.start_byte() {
            leaves.push(Leaf {
                start_row: node.start_position().row,
                end_row: node.end_position().row,
                start_column: node.start_position().column,
                comment: node.kind() == COMMENT,
            });
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_leaves(child, leaves);
    }
}

fn comment_groups(leaves: &[Leaf]) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut previous_token_row: Option<usize> = None;
    let mut index = 0;
    while let Some(leaf) = leaves.get(index) {
        if !leaf.comment {
            previous_token_row = Some(leaf.end_row);
            index = index.saturating_add(1);
            continue;
        }
        let trailing = previous_token_row == Some(leaf.start_row);
        let mut gap = if trailing {
            TRAILING_GROUP_GAP
        } else {
            LEADING_GROUP_GAP
        };
        while let Some(first) = leaves.get(index).filter(|candidate| candidate.comment) {
            let mut group = Group {
                comments: vec![*first],
                start_row: first.start_row,
                end_row: first.end_row,
                start_column: first.start_column,
            };
            index = index.saturating_add(1);
            while let Some(next) = leaves
                .get(index)
                .filter(|next| next.comment && next.start_row <= group.end_row.saturating_add(gap))
            {
                group.end_row = next.end_row;
                group.comments.push(*next);
                index = index.saturating_add(1);
            }
            groups.push(group);
            gap = LEADING_GROUP_GAP;
        }
        previous_token_row = None;
    }
    groups
}

fn comment_text(root: Node<'_>, source: &str, comment: &Leaf) -> String {
    let start = tree_sitter::Point::new(comment.start_row, comment.start_column);
    root.descendant_for_point_range(start, start)
        .filter(|node| node.kind() == COMMENT)
        .and_then(|node| node.utf8_text(source.as_bytes()).ok())
        .unwrap_or_default()
        .to_owned()
}

fn parse_directive(comment: &str) -> Option<NolintRange> {
    let text = comment.trim_start_matches(LEADING_MARKS);
    let rest = text.strip_prefix(DIRECTIVE)?;
    if !(rest.is_empty() || rest.starts_with(DIRECTIVE_ENDINGS) || rest.starts_with(':')) {
        return None;
    }
    let reason = text
        .split_once(REASON_SEPARATOR)
        .map(|(_, reason)| reason.trim().to_owned())
        .unwrap_or_default();
    let mut range = NolintRange {
        line: 0,
        last_line: 0,
        linters: Vec::new(),
        text: comment.trim_start_matches('/').trim().to_owned(),
        reason,
    };
    let Some(list) = text.strip_prefix(LINTER_LIST) else {
        return Some(range);
    };
    if list.starts_with(EVERY_LINTER) {
        return Some(range);
    }
    let list = list.split_once(REASON_SEPARATOR).map_or(list, |(list, _)| list);
    let linters: Vec<String> = list
        .split(LINTER_SEPARATOR)
        .map(|linter| linter.trim().to_lowercase())
        .collect();
    if !linters.iter().any(|linter| linter == EVERY_LINTER) {
        range.linters = linters;
    }
    Some(range)
}

fn expanded(root: Node<'_>, group: &Group, mut range: NolintRange) -> NolintRange {
    let target_row = group.end_row.saturating_add(1);
    let mut last_row = group.end_row;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let start = node.start_position();
        if start.row > target_row || node.end_position().row < target_row {
            continue;
        }
        let has_go_node = node.kind() != COMMENT && !WRAPPERS_WITHOUT_GO_NODE.contains(&node.kind());
        if start.row == target_row && start.column == group.start_column && has_go_node {
            last_row = last_row.max(node_end_row(root, node));
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    range.last_line = one_based(last_row);
    range
}

fn node_end_row(root: Node<'_>, node: Node<'_>) -> usize {
    if node.kind() != PACKAGE_CLAUSE {
        return node.end_position().row;
    }
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .map(|child| child.end_position().row)
        .max()
        .unwrap_or_else(|| node.end_position().row)
}
