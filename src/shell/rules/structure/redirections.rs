use std::collections::HashSet;

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    ASSIGNMENT, BODY_FIELD, COMMAND, COMMAND_NAME, COMMAND_SUBSTITUTION, CONCATENATION, DESCRIPTOR_FIELD,
    DESTINATION_FIELD, EXPANSIONS, FILE_REDIRECT, LIST, PIPELINE, REDIRECT_FIELD, REDIRECTED, STRING,
    ancestors, children, descendants, field_children, is_statement, named_children, text,
};
use crate::shell::words::{literal, oversimplify};

const STDOUT: &str = "1";
const NULL_DEVICE_PREFIX: &str = "/dev/";
const WRITE: &str = ">";
const APPEND: &str = ">>";
const READ: &str = "<";
const DUPLICATE: &str = ">&";
const HARMLESS_COMMANDS: &[&str] = &["echo", "mapfile", "printf", "sponge"];
const PATH_SEPARATOR: char = '/';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() == COMMAND_SUBSTITUTION {
        check_captured_redirection(node, context.source, findings);
    }
    if is_pipeline_unit(node) {
        check_same_file(node, context.source, findings);
    }
}

// SC2327 y SC2328: `checkExpansionWithRedirection`.
fn check_captured_redirection(substitution: Node<'_>, source: &[u8], findings: &mut Findings) {
    let [statement] = named_children(substitution)[..] else {
        return;
    };
    for redirect in last_stage_redirects(statement) {
        if redirect.kind() != FILE_REDIRECT {
            continue;
        }
        let descriptor = redirect
            .child_by_field_name(DESCRIPTOR_FIELD)
            .map_or("", |descriptor| text(descriptor, source));
        let operator = redirect_operator(redirect, source);
        let destination = redirect.child_by_field_name(DESTINATION_FIELD);
        let duplicates = operator == DUPLICATE
            && destination.is_some_and(|target| {
                text(target, source)
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '-')
            });
        if duplicates {
            let target = destination.map_or("", |target| text(target, source));
            if target == STDOUT || descriptor == STDOUT {
                return;
            }
            if descriptor.is_empty() {
                report_capture(substitution, redirect, source, findings);
                return;
            }
        } else if [WRITE, APPEND].contains(&operator) && (descriptor.is_empty() || descriptor == STDOUT) {
            report_capture(substitution, redirect, source, findings);
            return;
        }
    }
}

fn report_capture(substitution: Node<'_>, redirect: Node<'_>, source: &[u8], findings: &mut Findings) {
    findings.report(ShellRule::Sc2327, substitution, text(substitution, source));
    findings.report(ShellRule::Sc2328, redirect, text(redirect, source));
}

fn redirect_operator<'source>(redirect: Node<'_>, source: &'source [u8]) -> &'source str {
    children(redirect)
        .into_iter()
        .find(|child| !child.is_named())
        .map_or("", |operator| text(operator, source))
}

// tree-sitter cuelga del pipeline entero la redirección del último comando.
fn last_stage_redirects(statement: Node<'_>) -> Vec<Node<'_>> {
    let (body, outer) = if statement.kind() == REDIRECTED {
        (
            statement.child_by_field_name(BODY_FIELD),
            field_children(statement, REDIRECT_FIELD),
        )
    } else {
        (Some(statement), Vec::new())
    };
    if body.is_some_and(|body| body.kind() == LIST) {
        return Vec::new();
    }
    let mut redirects = Vec::new();
    if let Some(pipeline) = body.filter(|body| body.kind() == PIPELINE)
        && let Some(last) = named_children(pipeline).last().copied()
        && last.kind() == REDIRECTED
    {
        redirects.extend(field_children(last, REDIRECT_FIELD));
    }
    redirects.extend(outer);
    redirects
}

// Lo que ShellCheck ve como un `T_Pipeline` con redirecciones: el pipeline con las suyas, o un comando
// redirigido que no es parte de uno.
fn is_pipeline_unit(node: Node<'_>) -> bool {
    let parent_kind = node.parent().map(|parent| parent.kind());
    match node.kind() {
        PIPELINE => parent_kind != Some(REDIRECTED),
        REDIRECTED => parent_kind != Some(PIPELINE),
        _ => false,
    }
}

// tree-sitter cuelga de toda la lista la redirección del último comando de un `a && b < f`: los
// comandos de antes no son parte del pipeline.
fn pipeline_scope(unit: Node<'_>) -> (Vec<Node<'_>>, Vec<Node<'_>>) {
    let (mut redirects, mut body) = if unit.kind() == REDIRECTED {
        (
            field_children(unit, REDIRECT_FIELD),
            unit.child_by_field_name(BODY_FIELD).unwrap_or(unit),
        )
    } else {
        (Vec::new(), unit)
    };
    while body.kind() == LIST {
        match named_children(body).last().copied() {
            Some(last) => body = last,
            None => break,
        }
    }
    let stages = if body.kind() == PIPELINE {
        named_children(body)
    } else {
        vec![body]
    };
    for stage in stages.iter().filter(|stage| stage.kind() == REDIRECTED) {
        redirects.extend(field_children(*stage, REDIRECT_FIELD));
    }
    (stages, redirects)
}

// SC2094: `checkRedirectToSame`.
fn check_same_file(unit: Node<'_>, source: &[u8], findings: &mut Findings) {
    let (stages, redirects) = pipeline_scope(unit);
    let files: Vec<Node<'_>> = redirects
        .iter()
        .copied()
        .filter(|redirect| {
            redirect.kind() == FILE_REDIRECT
                && [WRITE, READ, APPEND].contains(&redirect_operator(*redirect, source))
        })
        .filter_map(|redirect| redirect.child_by_field_name(DESTINATION_FIELD))
        .collect();
    if files.is_empty() {
        return;
    }
    let words: Vec<Node<'_>> = stages
        .iter()
        .chain(redirects.iter())
        .flat_map(|node| descendants(*node))
        .filter(|node| is_whole_word(*node))
        .collect();
    let conflicts = files.iter().flat_map(|file| {
        words
            .iter()
            .filter(|word| {
                word.id() != file.id()
                    && text(**word, source) == text(*file, source)
                    && is_conflict(*file, **word, source)
            })
            .flat_map(|word| [*word, *file])
    });
    let mut reported = HashSet::new();
    for occurrence in conflicts {
        if reported.insert(occurrence.id()) {
            findings.report(ShellRule::Sc2094, occurrence, text(occurrence, source));
        }
    }
}

fn is_whole_word(node: Node<'_>) -> bool {
    if !node.is_named() || node.kind() == COMMAND_NAME {
        return false;
    }
    node.parent().is_some_and(|parent| {
        let is_part = [CONCATENATION, STRING].contains(&parent.kind()) || EXPANSIONS.contains(&parent.kind());
        !is_part && (node.child_count() == 0 || [STRING, CONCATENATION].contains(&node.kind()))
    })
}

fn redirect_of(word: Node<'_>) -> Option<Node<'_>> {
    word.parent().filter(|parent| {
        parent.kind() == FILE_REDIRECT
            && parent
                .child_by_field_name(DESTINATION_FIELD)
                .is_some_and(|destination| destination.id() == word.id())
    })
}

fn has_operator(word: Node<'_>, operators: &[&str], source: &[u8]) -> bool {
    redirect_of(word).is_some_and(|redirect| operators.contains(&redirect_operator(redirect, source)))
}

fn is_conflict(file: Node<'_>, word: Node<'_>, source: &[u8]) -> bool {
    let both_input = has_operator(file, &[READ], source) && has_operator(word, &[READ], source);
    let both_output =
        has_operator(file, &[WRITE, APPEND], source) && has_operator(word, &[WRITE, APPEND], source);
    !both_input
        && !both_output
        && !oversimplify(file, source).starts_with(NULL_DEVICE_PREFIX)
        && !is_harmless(file, source)
        && !is_harmless(word, source)
        && !is_in_plain_assignment(word)
}

fn closest_command(word: Node<'_>) -> Option<Command<'_>> {
    let node = ancestors(word).find(|ancestor| [COMMAND, REDIRECTED].contains(&ancestor.kind()))?;
    let mut simple = node;
    while [REDIRECTED, PIPELINE, LIST].contains(&simple.kind()) {
        simple = if simple.kind() == REDIRECTED {
            simple.child_by_field_name(BODY_FIELD)?
        } else {
            *named_children(simple).last()?
        };
    }
    Command::of(simple)
}

fn is_harmless(word: Node<'_>, source: &[u8]) -> bool {
    closest_command(word)
        .and_then(|command| command.first_word())
        .and_then(|name| literal(name, source))
        .is_some_and(|name| {
            let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or(&name);
            HARMLESS_COMMANDS.contains(&basename)
        })
}

fn is_in_plain_assignment(word: Node<'_>) -> bool {
    ancestors(word)
        .find(|ancestor| ancestor.kind() == ASSIGNMENT || ancestor.kind() == COMMAND)
        .is_some_and(|holder| holder.kind() == ASSIGNMENT && is_statement(holder))
}
