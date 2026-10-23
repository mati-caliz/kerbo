use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::nodes::{COMMENT, STATEMENT_BLOCK, text};
use super::paths::case_statements;
use super::token_index::TokenIndex;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const SWITCH_BODY: &str = "switch_body";
const CASES: &[&str] = &["switch_case", "switch_default"];
const LINE_COMMENT: &str = "//";
const BLOCK_START: &str = "/*";
const BLOCK_END: &str = "*/";
const CLOSING_BRACE: &str = "}";
const BLANK_LINE_GAP: usize = 1;

static FALLTHROUGH_COMMENT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?i)falls?\s?through").ok());
static DIRECTIVE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(eslint(?:-env|-enable|-disable(?:(?:-next)?-line)?)?|exported|globals?)(?:\s|$)").ok()
});

// `no-fallthrough`: un `case` que sigue de largo al siguiente sin `break` ni comentario que lo avise.
pub fn fallthroughs(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let reachability = context.paths();
    let tokens = TokenIndex::new(context.root);
    let mut found = Vec::new();
    for body in context.of_kind(&[SWITCH_BODY]) {
        let mut cursor = body.walk();
        let cases: Vec<Node<'_>> = body
            .named_children(&mut cursor)
            .filter(|child| CASES.contains(&child.kind()))
            .collect();
        for pair in cases.windows(2) {
            let [previous, case] = pair else {
                continue;
            };
            let statements = case_statements(*previous);
            let falls = reachability.reachable_at_exit(*previous)
                && (!statements.is_empty() || has_blank_line_before(*previous, *case));
            if falls && !has_fallthrough_comment(&statements, *case, &tokens, context.source) {
                found.push(report(EslintRule::NoFallthrough, *case, ""));
            }
        }
    }
    found
}

fn has_blank_line_before(previous: Node<'_>, case: Node<'_>) -> bool {
    let mut cursor = previous.walk();
    let end_row = previous
        .children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .last()
        .map_or_else(|| previous.end_position().row, |last| last.end_position().row);
    case.start_position().row > end_row.saturating_add(BLANK_LINE_GAP)
}

fn has_fallthrough_comment(
    statements: &[Node<'_>],
    case: Node<'_>,
    tokens: &TokenIndex<'_>,
    source: &[u8],
) -> bool {
    let in_block = match statements {
        [block] if block.kind() == STATEMENT_BLOCK => closing_brace(*block)
            .and_then(|brace| tokens.comments_before(brace.start_byte()).pop())
            .is_some_and(|comment| is_fallthrough_comment(comment, source)),
        _ => false,
    };
    in_block
        || tokens
            .comments_before(case.start_byte())
            .pop()
            .is_some_and(|comment| is_fallthrough_comment(comment, source))
}

fn closing_brace(block: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = block.walk();
    block
        .children(&mut cursor)
        .filter(|child| child.kind() == CLOSING_BRACE)
        .last()
}

fn is_fallthrough_comment(comment: Node<'_>, source: &[u8]) -> bool {
    let raw = text(comment, source);
    let value = raw
        .strip_prefix(LINE_COMMENT)
        .or_else(|| {
            raw.strip_prefix(BLOCK_START)
                .map(|inner| inner.strip_suffix(BLOCK_END).unwrap_or(inner))
        })
        .unwrap_or(raw);
    FALLTHROUGH_COMMENT
        .as_ref()
        .is_some_and(|pattern| pattern.is_match(value))
        && !DIRECTIVE
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(value.trim()))
}
