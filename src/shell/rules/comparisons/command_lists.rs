use tree_sitter::Node;

use super::test_operators::operands_exclude_each_other;
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    AND, BACKGROUND, BODY_FIELD, DESCRIPTOR_FIELD, FILE_REDIRECT, LIST, OR, PIPELINE, REDIRECT_FIELD,
    REDIRECTED, TEST, children, field_children, named_children, text,
};
use crate::shell::test_expression::{Condition, operator_text, parse};
use crate::shell::word_pieces::{has_glob, is_constant, pieces};

const NOT_EQUAL_OPERATORS: &[&str] = &["-ne", "!="];
const TEST_COMMAND: &str = "test";
const PIPE: &str = "|";
const STDERR: &str = "2";
const COMPARISON_REDIRECTS: &[&str] = &[">", "<"];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    match node.kind() {
        LIST => check_repeated_subject(node, source, findings),
        PIPELINE => check_pipes_between_tests(node, source, findings),
        REDIRECTED => check_comparison_redirects(node, source, findings),
        _ => {}
    }
    check_backgrounded_test(node, source, findings);
}

struct Operands<'tree, 'source> {
    subject: &'source str,
    operator: &'source str,
    constant: Node<'tree>,
}

// SC2252 y SC2333: `checkOrNeq` y `checkAndEq` entre comandos.
fn check_repeated_subject(list: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(operator) = list_operator(list, source) else {
        return;
    };
    let list_kind = text(operator, source);
    if ![AND, OR].contains(&list_kind) {
        return;
    }
    let [left, right] = named_children(list)[..] else {
        return;
    };
    let (Some(first), Some(second)) = (
        operands(left, list_kind, source),
        operands(right, list_kind, source),
    ) else {
        return;
    };
    if first.operator != second.operator
        || first.subject != second.subject
        || text(first.constant, source) == text(second.constant, source)
    {
        return;
    }
    if list_kind == OR {
        let any_glob =
            has_glob(&pieces(first.constant, source)) || has_glob(&pieces(second.constant, source));
        if NOT_EQUAL_OPERATORS.contains(&first.operator) && !any_glob {
            findings.report(ShellRule::Sc2252, operator, list_kind);
        }
    } else if operands_exclude_each_other(first.operator, first.constant, second.constant, source) {
        findings.report(ShellRule::Sc2333, operator, list_kind);
    }
}

fn list_operator<'tree>(list: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    children(list)
        .into_iter()
        .find(|child| !child.is_named() && [AND, OR].contains(&text(*child, source)))
}

fn operands<'tree, 'source>(
    node: Node<'tree>,
    list_kind: &str,
    source: &'source [u8],
) -> Option<Operands<'tree, 'source>> {
    match node.kind() {
        LIST if list_operator(node, source).is_some_and(|operator| text(operator, source) == list_kind) => {
            operands(*named_children(node).first()?, list_kind, source)
        }
        REDIRECTED => operands(node.child_by_field_name(BODY_FIELD)?, list_kind, source),
        TEST => {
            let Condition::Binary(operator, left, right) = parse(node, source)? else {
                return None;
            };
            let operator = operator_text(operator, source);
            match (
                is_constant(&pieces(left, source)),
                is_constant(&pieces(right, source)),
            ) {
                (true, false) => Some(Operands {
                    subject: text(right, source),
                    operator,
                    constant: left,
                }),
                (false, true) => Some(Operands {
                    subject: text(left, source),
                    operator,
                    constant: right,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

// SC2266: `checkBadTestAndOr`.
fn check_pipes_between_tests(pipeline: Node<'_>, source: &[u8], findings: &mut Findings) {
    let parts = children(pipeline);
    for (index, part) in parts.iter().enumerate() {
        if part.is_named() || text(*part, source) != PIPE {
            continue;
        }
        let before = index
            .checked_sub(1)
            .and_then(|previous| parts.get(previous))
            .copied();
        let after = parts.get(index.saturating_add(1)).copied();
        if [before, after]
            .into_iter()
            .flatten()
            .any(|stage| is_test_command(stage, source))
        {
            findings.report(ShellRule::Sc2266, *part, PIPE);
        }
    }
}

fn is_test_command(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        TEST => true,
        REDIRECTED => node
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|body| is_test_command(body, source)),
        _ => Command::of(node).is_some_and(|command| command.is_command(TEST_COMMAND, source)),
    }
}

// SC2265: `checkBadTestAndOr`, `[ a ] & [ b ]`.
fn check_backgrounded_test(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(ampersand) = node.next_sibling() else {
        return;
    };
    if ampersand.is_named() || text(ampersand, source) != BACKGROUND || !node.is_named() {
        return;
    }
    if ends_in_test(node, source) {
        findings.report(ShellRule::Sc2265, ampersand, BACKGROUND);
    }
}

fn ends_in_test(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        LIST | PIPELINE => named_children(node)
            .last()
            .is_some_and(|last| ends_in_test(*last, source)),
        _ => is_test_command(node, source),
    }
}

// SC2065: `checkTestRedirects`.
fn check_comparison_redirects(statement: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_test = statement
        .child_by_field_name(BODY_FIELD)
        .and_then(Command::of)
        .is_some_and(|command| command.is_command(TEST_COMMAND, source));
    if !is_test {
        return;
    }
    for redirect in field_children(statement, REDIRECT_FIELD) {
        if redirect.kind() != FILE_REDIRECT {
            continue;
        }
        let descriptor = redirect
            .child_by_field_name(DESCRIPTOR_FIELD)
            .map_or("", |descriptor| text(descriptor, source));
        let compares = children(redirect)
            .into_iter()
            .any(|child| !child.is_named() && COMPARISON_REDIRECTS.contains(&text(child, source)));
        if compares && descriptor != STDERR {
            findings.report(ShellRule::Sc2065, redirect, text(redirect, source));
        }
    }
}
