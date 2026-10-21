use tree_sitter::Node;

use crate::fix::{Fix, TextEdit, TextPoint};
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::environment::SPECIAL_WITHOUT_SPACES;
use crate::shell::flow;
use crate::shell::parameters::{is_array_expansion, is_counting, is_quoted_alternative, modifier, reference};
use crate::shell::quoting::{Strictness, is_quote_free, used_as_command_name};
use crate::shell::syntax::{
    COMMAND_NAME, CONCATENATION, EXPANSIONS, RAW_STRING, STRING, WORD, children, descendants, text,
    token_start,
};
use crate::shell::words::braced_content;

const DOUBLE_QUOTE: &str = "\"";
const DEFAULT_ASSIGNMENTS: &[&str] = &["=", ":="];
const NO_OP: &str = ":";
const PATH_SEPARATOR: char = '/';
const ASSIGNMENT_SIGN: char = '=';

pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let candidates: Vec<Node<'_>> = descendants(context.root)
        .into_iter()
        .filter(|node| EXPANSIONS.contains(&node.kind()) && needs_quoting(*node, context))
        .collect();
    if candidates.is_empty() {
        return;
    }
    let clean = flow::clean_expansions(context, &candidates);
    for expansion in candidates {
        if clean.contains(&expansion.start_byte()) {
            continue;
        }
        report(expansion, context, findings);
    }
}

fn needs_quoting(node: Node<'_>, context: &Context<'_, '_>) -> bool {
    let braced = braced_content(node, context.source);
    let name = reference(&braced);
    !is_array_expansion(&braced)
        && !is_counting(&braced)
        && !is_quoted_alternative(&braced)
        && !SPECIAL_WITHOUT_SPACES.contains(&name.as_str())
        && !is_quote_free(node, context, Strictness::Lenient)
        && !used_as_command_name(node, context.source)
        && !may_conflict_with_assignment_fix(node, context.source)
}

fn report(expansion: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let braced = braced_content(expansion, source);
    let modifier = modifier(&braced);
    let default_assignment = DEFAULT_ASSIGNMENTS
        .iter()
        .any(|prefix| modifier.starts_with(prefix));
    if default_assignment && is_argument_to_no_op(expansion, source) {
        findings.report(ShellRule::Sc2223, expansion, text(expansion, source));
        return;
    }
    let (_, start) = token_start(expansion.start_byte(), expansion.start_position(), source);
    let quote_at = |point| TextEdit {
        start: TextPoint::from_point(point),
        end: TextPoint::from_point(point),
        replacement: DOUBLE_QUOTE.to_owned(),
    };
    let fix = Fix::from_edits(source, vec![quote_at(start), quote_at(expansion.end_position())]);
    findings.report_fixed(ShellRule::Sc2086, expansion, (text(expansion, source), fix));
}

fn may_conflict_with_assignment_fix(node: Node<'_>, source: &[u8]) -> bool {
    let Some(word) = node.parent().filter(|parent| parent.kind() == CONCATENATION) else {
        return false;
    };
    let parts = children(word);
    let starts_assignment = parts.first() == Some(&node)
        && parts
            .get(1)
            .is_some_and(|next| next.kind() == WORD && text(*next, source).starts_with(ASSIGNMENT_SIGN));
    starts_assignment && word.parent().is_some_and(|parent| parent.kind() == COMMAND_NAME)
}

fn is_argument_to_no_op(node: Node<'_>, source: &[u8]) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if [STRING, RAW_STRING, CONCATENATION].contains(&parent.kind()) {
            current = parent;
            continue;
        }
        return Command::of(parent).is_some_and(|command| {
            command.arguments().contains(&current)
                && command.name(source).is_some_and(|name| {
                    name == NO_OP
                        || name
                            .strip_suffix(NO_OP)
                            .is_some_and(|prefix| prefix.ends_with(PATH_SEPARATOR))
                })
        });
    }
    false
}
