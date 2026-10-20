mod branches;

use tree_sitter::Node;

use super::{FNV_OFFSET, fnv, image_hash};
use crate::syntax::{descendants, named_children, one_based};

const COMMENT_KINDS: &[&str] = &["comment", "html_comment"];
const DECLARED_FUNCTIONS: &[&str] = &["function_declaration", "generator_function_declaration"];
const ASSIGNED_FUNCTIONS: &[&str] = &["function_expression", "generator_function", "arrow_function"];
const PARENTHESIZED: &str = "parenthesized_expression";
const OPEN_BRACE: &str = "{";
const CLOSE_BRACE: &str = "}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatedKind {
    Function,
    Branch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatedBlock {
    pub kind: RepeatedKind,
    pub line: usize,
    pub end_line: usize,
    pub original_line: usize,
    pub lines: usize,
    pub content: u64,
}

#[derive(Debug, Clone, Copy)]
struct Token {
    image: u64,
    line: usize,
    end_line: usize,
}

struct Context<'source> {
    source: &'source [u8],
    min_lines: usize,
}

// Calca no-identical-functions y no-duplicated-branches de sonarjs: dos nodos son iguales si son
// del mismo tipo y tienen los mismos tokens, sin comentarios.
pub fn repeated_blocks(root: Node<'_>, source: &[u8], limit: usize) -> Vec<RepeatedBlock> {
    let context = Context {
        source,
        min_lines: limit.saturating_add(1),
    };
    let nodes = descendants(root);
    let mut found = repeated_functions(&context, &nodes);
    for node in nodes {
        match node.kind() {
            "if_statement" => found.extend(branches::repeated_if_branches(&context, node)),
            "switch_statement" => found.extend(branches::repeated_cases(&context, node)),
            _ => {}
        }
    }
    found
}

fn tokens(node: Node<'_>, source: &[u8]) -> Vec<Token> {
    descendants(node)
        .into_iter()
        .filter(|leaf| leaf.child_count() == 0 && leaf.start_byte() < leaf.end_byte())
        .filter(|leaf| !COMMENT_KINDS.contains(&leaf.kind()))
        .map(|leaf| Token {
            image: image_hash(leaf.utf8_text(source).unwrap_or_default()),
            line: one_based(leaf.start_position().row),
            end_line: one_based(leaf.end_position().row),
        })
        .collect()
}

fn equivalent(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    left.kind() == right.kind() && same_images(&tokens(left, source), &tokens(right, source))
}

fn all_equivalent(left: &[Node<'_>], right: &[Node<'_>], source: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(&first, &second)| equivalent(first, second, source))
}

fn same_images(left: &[Token], right: &[Token]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(first, second)| first.image == second.image)
}

fn content_hash(nodes: &[Node<'_>], source: &[u8]) -> u64 {
    fnv(
        FNV_OFFSET,
        nodes
            .iter()
            .flat_map(|&node| tokens(node, source))
            .map(|token| token.image),
    )
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match named_children(current)
            .into_iter()
            .find(|child| !COMMENT_KINDS.contains(&child.kind()))
        {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}

fn parent_skipping_parentheses(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    Some(parent)
}

fn is_checked_function(node: Node<'_>) -> bool {
    let kind = node.kind();
    if DECLARED_FUNCTIONS.contains(&kind) {
        return true;
    }
    if kind == "method_definition" {
        return node.parent().is_some_and(|parent| parent.kind() == "class_body");
    }
    ASSIGNED_FUNCTIONS.contains(&kind)
        && parent_skipping_parentheses(node).is_some_and(|parent| {
            parent.kind() == "variable_declarator"
                && parent
                    .child_by_field_name("value")
                    .is_some_and(|value| unparenthesized(value).id() == node.id())
        })
}

struct Candidate<'tree> {
    function: Node<'tree>,
    body: Node<'tree>,
}

fn repeated_functions(context: &Context<'_>, nodes: &[Node<'_>]) -> Vec<RepeatedBlock> {
    let candidates: Vec<Candidate<'_>> = nodes
        .iter()
        .copied()
        .filter(|&node| is_checked_function(node))
        .filter_map(|function| {
            let body = unparenthesized(function.child_by_field_name("body")?);
            is_big_enough(&tokens(body, context.source), context.min_lines)
                .then_some(Candidate { function, body })
        })
        .collect();
    let mut found = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let original = candidates
            .iter()
            .take(index)
            .find(|earlier| equivalent(candidate.body, earlier.body, context.source));
        if let Some(earlier) = original {
            found.push(RepeatedBlock {
                kind: RepeatedKind::Function,
                line: report_line(candidate.function, context.source),
                end_line: one_based(candidate.function.end_position().row),
                original_line: one_based(earlier.function.start_position().row),
                lines: block_lines(candidate.body, context.source),
                content: content_hash(&[candidate.body], context.source),
            });
        }
    }
    found
}

fn is_big_enough(body: &[Token], min_lines: usize) -> bool {
    let inner = trimmed_braces(body);
    match (inner.first(), inner.last()) {
        (Some(first), Some(last)) => last.end_line.saturating_sub(first.line).saturating_add(1) >= min_lines,
        _ => false,
    }
}

fn trimmed_braces(body: &[Token]) -> &[Token] {
    let open = image_hash(OPEN_BRACE);
    let close = image_hash(CLOSE_BRACE);
    let without_open = match body.split_first() {
        Some((first, rest)) if first.image == open => rest,
        _ => body,
    };
    match without_open.split_last() {
        Some((last, rest)) if last.image == close => rest,
        _ => without_open,
    }
}

fn block_lines(body: Node<'_>, source: &[u8]) -> usize {
    let inner_tokens = tokens(body, source);
    let inner = trimmed_braces(&inner_tokens);
    match (inner.first(), inner.last()) {
        (Some(first), Some(last)) => last.end_line.saturating_sub(first.line).saturating_add(1),
        _ => 0,
    }
}

fn report_line(function: Node<'_>, source: &[u8]) -> usize {
    let anchor = match function.kind() {
        "method_definition" => function.child_by_field_name("name").map(|name| {
            if name.kind() == "computed_property_name" {
                named_children(name).first().copied().unwrap_or(name)
            } else {
                name
            }
        }),
        "arrow_function" => token_child(function, "=>"),
        kind if DECLARED_FUNCTIONS.contains(&kind) => function
            .child_by_field_name("name")
            .or_else(|| first_token(function, "function", source)),
        _ => first_token(function, "function", source),
    };
    one_based(anchor.unwrap_or(function).start_position().row)
}

fn token_child<'tree>(node: Node<'tree>, text: &str) -> Option<Node<'tree>> {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .find(|child| !child.is_named() && child.kind() == text)
}

fn first_token<'tree>(node: Node<'tree>, text: &str, source: &[u8]) -> Option<Node<'tree>> {
    descendants(node)
        .into_iter()
        .find(|leaf| leaf.child_count() == 0 && leaf.utf8_text(source).ok() == Some(text))
}

#[cfg(test)]
mod tests;
