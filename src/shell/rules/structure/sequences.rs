use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    BODY_FIELD, COMMAND_SUBSTITUTION, COMPOUND, CONDITION_FIELD, DESTINATION_FIELD, DO_GROUP, ELIF_CLAUSE,
    ELSE_CLAUSE, FILE_REDIRECT, IF, LIST, PIPELINE, PROGRAM, REDIRECT_FIELD, REDIRECTED, SUBSHELL, WHILE,
    children, field_children, is_arithmetic_command, is_backgrounded, named_children, text,
};
use crate::shell::words::oversimplify;

const CD: &str = "cd";
const CD_REVERTS: &[&str] = &["..", "-"];
const CD_REVERT_WORDS: usize = 2;
const APPEND: &str = ">>";
const THEN: &str = "then";
const ELSE: &str = "else";
const MINIMUM_APPENDS: usize = 3;
const BRANCH_ENDS: &[&str] = &["fi", "elif", "else"];

// `getCommandSequences` de ShellCheck, con `checkCdAndBack` (SC2103) y `checkMultipleAppends`
// (SC2129) sobre cada secuencia.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    for sequence in command_sequences(node, source) {
        if !context.has_set_e {
            check_cd_and_back(&sequence, source, findings);
        }
        check_multiple_appends(&sequence, source, findings);
    }
}

fn command_sequences<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Vec<Node<'tree>>> {
    match node.kind() {
        PROGRAM | SUBSHELL | DO_GROUP | COMMAND_SUBSTITUTION => vec![named_children(node)],
        COMPOUND if !is_arithmetic_command(node, source) => vec![named_children(node)],
        WHILE => vec![conditions(node)],
        IF | ELIF_CLAUSE => vec![conditions(node), branch_body(node, THEN, source)],
        ELSE_CLAUSE => vec![branch_body(node, ELSE, source)],
        _ => Vec::new(),
    }
}

fn conditions(node: Node<'_>) -> Vec<Node<'_>> {
    field_children(node, CONDITION_FIELD)
        .into_iter()
        .filter(Node::is_named)
        .collect()
}

fn branch_body<'tree>(node: Node<'tree>, start: &str, source: &[u8]) -> Vec<Node<'tree>> {
    children(node)
        .into_iter()
        .skip_while(|child| text(*child, source) != start)
        .skip(1)
        .take_while(|child| !BRANCH_ENDS.contains(&text(*child, source)))
        .filter(|child| {
            child.is_named()
                && ![ELIF_CLAUSE, ELSE_CLAUSE, crate::shell::syntax::COMMENT].contains(&child.kind())
        })
        .collect()
}

fn check_cd_and_back(sequence: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let candidates: Vec<Node<'_>> = sequence
        .iter()
        .copied()
        .filter(|statement| !is_backgrounded(*statement, source) && is_cd(*statement, source))
        .collect();
    let revert = candidates.windows(2).find_map(|pair| match pair {
        [first, second] if is_cd_revert(*second, source) && !is_cd_revert(*first, source) => Some(*second),
        _ => None,
    });
    if let Some(revert) = revert {
        findings.report(ShellRule::Sc2103, revert, CD);
    }
}

fn command_of(statement: Node<'_>) -> Option<Command<'_>> {
    if statement.kind() == REDIRECTED {
        Command::of(statement.child_by_field_name(BODY_FIELD)?)
    } else {
        Command::of(statement)
    }
}

fn is_cd(statement: Node<'_>, source: &[u8]) -> bool {
    command_of(statement).is_some_and(|command| command.is_exactly(CD, source))
}

fn is_cd_revert(statement: Node<'_>, source: &[u8]) -> bool {
    command_of(statement).is_some_and(|command| {
        command.words.len() == CD_REVERT_WORDS
            && command
                .words
                .last()
                .is_some_and(|word| CD_REVERTS.contains(&oversimplify(*word, source).as_str()))
    })
}

fn check_multiple_appends(sequence: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let mut first_in_group: Option<(String, Node<'_>)> = None;
    let mut group_size = 0_usize;
    for statement in sequence {
        let target = appended_file(*statement, source);
        let continues = matches!(
            (&first_in_group, &target),
            (Some((current, _)), Some((file, _))) if current == file
        );
        if continues {
            group_size = group_size.saturating_add(1);
        } else {
            report_appends(first_in_group.as_ref(), group_size, findings);
            first_in_group = target;
            group_size = 1;
        }
    }
    report_appends(first_in_group.as_ref(), group_size, findings);
}

fn report_appends(group: Option<&(String, Node<'_>)>, size: usize, findings: &mut Findings) {
    if let Some((file, start)) = group
        && size >= MINIMUM_APPENDS
    {
        findings.report(ShellRule::Sc2129, *start, file);
    }
}

// El archivo del primer `>>` del comando y dónde empieza (el último del pipeline, si es uno).
fn appended_file<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<(String, Node<'tree>)> {
    if is_backgrounded(statement, source) {
        return None;
    }
    let mut redirects = Vec::new();
    let mut current = statement;
    if current.kind() == PIPELINE {
        current = *named_children(current).last()?;
    }
    let mut start = current;
    if current.kind() == REDIRECTED {
        redirects.push(field_children(current, REDIRECT_FIELD));
        let body = current.child_by_field_name(BODY_FIELD)?;
        if body.kind() == LIST {
            return None;
        }
        start = body;
        if body.kind() == PIPELINE {
            let last = *named_children(body).last()?;
            start = last;
            if last.kind() == REDIRECTED {
                redirects.insert(0, field_children(last, REDIRECT_FIELD));
                start = last.child_by_field_name(BODY_FIELD).unwrap_or(last);
            }
        }
    }
    let file = redirects
        .into_iter()
        .flatten()
        .filter(|redirect| redirect.kind() == FILE_REDIRECT)
        .find(|redirect| {
            children(*redirect)
                .into_iter()
                .any(|child| !child.is_named() && text(child, source) == APPEND)
        })?
        .child_by_field_name(DESTINATION_FIELD)?;
    Some((text(file, source).to_owned(), start))
}
