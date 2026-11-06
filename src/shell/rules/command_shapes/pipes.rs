use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Findings, Position};
use crate::shell::syntax::{
    BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, DESCRIPTOR_FIELD, DESTINATION_FIELD, FILE_REDIRECT,
    HEREDOC_REDIRECT, HERESTRING, LIST, PIPELINE, PROCESS_SUBSTITUTION, REDIRECT_FIELD, REDIRECTED, children,
    descendants, field_children, named_children, text,
};

const NON_READING_COMMANDS: &[&str] = &[
    "alias", "basename", "bg", "cal", "cd", "chgrp", "chmod", "chown", "cp", "du", "echo", "export", "fg",
    "fuser", "getconf", "getopt", "getopts", "ipcrm", "ipcs", "jobs", "kill", "ln", "ls", "locale", "mv",
    "printf", "ps", "pwd", "renice", "rm", "rmdir", "set", "sleep", "touch", "trap", "ulimit", "unalias",
    "uname",
];
const INTERACTIVE_COMMANDS: &[&str] = &["cp", "mv", "rm"];
const INTERACTIVE_FLAGS: &[&str] = &["i", "interactive"];
const DU: &str = "du";
const DU_READING_FLAGS: &[&str] = &["exclude-from", "files0-from"];
const PIPE: &str = "|";
const PIPE_WITH_STDERR: &str = "|&";
const OPEN_INPUT_SUBSTITUTION: &str = "<(";
const OPEN_OUTPUT_SUBSTITUTION: &str = ">(";
const CLOSE_DESCRIPTOR: &str = "-";
const PATH_SEPARATOR: char = '/';
const STDIN: u32 = 0;
const STDOUT: u32 = 1;
const STDERR: u32 = 2;
const READ: &str = "<";
const WRITE: &str = ">";
const APPEND: &str = ">>";
const CLOBBER: &str = ">|";
const DUPLICATE_OUTPUT: &str = ">&";
const WRITE_BOTH: &str = "&>";
const BOTH_DESCRIPTORS_MARK: char = '&';
const HEREDOC_START: &str = "heredoc_start";
const APPEND_BOTH: &str = "&>>";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pipe {
    None,
    Stdout,
    StdoutAndStderr,
}

struct Stage<'tree> {
    body: Node<'tree>,
    redirects: Vec<Node<'tree>>,
    input: Pipe,
    output: Pipe,
}

// SC2216, SC2217, SC2259, SC2260 y SC2261: `checkPipeToNowhere`.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    for stage in stages_of(node, source) {
        check_stage(&stage, source, findings);
    }
}

// tree-sitter cuelga del pipeline o de la lista entera las redirecciones del último comando.
fn stages_of<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Stage<'tree>> {
    let parent_kind = node.parent().map(|parent| parent.kind());
    match node.kind() {
        REDIRECTED if parent_kind != Some(PIPELINE) => {
            let trailing = field_children(node, REDIRECT_FIELD);
            let Some(body) = node.child_by_field_name(BODY_FIELD) else {
                return vec![single_stage(node, trailing)];
            };
            stages_with_trailing(body, trailing, source)
        }
        PIPELINE if parent_kind != Some(REDIRECTED) => stages_with_trailing(node, Vec::new(), source),
        COMMAND if ![Some(REDIRECTED), Some(PIPELINE)].contains(&parent_kind) => {
            let redirects = field_children(node, REDIRECT_FIELD);
            if redirects.is_empty() {
                Vec::new()
            } else {
                vec![single_stage(node, redirects)]
            }
        }
        _ => Vec::new(),
    }
}

const fn single_stage<'tree>(body: Node<'tree>, redirects: Vec<Node<'tree>>) -> Stage<'tree> {
    Stage {
        body,
        redirects,
        input: Pipe::None,
        output: Pipe::None,
    }
}

fn stages_with_trailing<'tree>(
    body: Node<'tree>,
    trailing: Vec<Node<'tree>>,
    source: &[u8],
) -> Vec<Stage<'tree>> {
    match body.kind() {
        LIST => named_children(body)
            .last()
            .map_or_else(Vec::new, |last| stages_with_trailing(*last, trailing, source)),
        PIPELINE => pipeline_stages(body, trailing, source),
        _ => vec![single_stage(body, trailing)],
    }
}

fn pipeline_stages<'tree>(
    pipeline: Node<'tree>,
    trailing: Vec<Node<'tree>>,
    source: &[u8],
) -> Vec<Stage<'tree>> {
    let mut stages: Vec<Stage<'tree>> = Vec::new();
    let mut pending_input = Pipe::None;
    for child in children(pipeline) {
        if child.is_named() {
            let (body, redirects) = split_stage(child);
            stages.push(Stage {
                body,
                redirects,
                input: pending_input,
                output: Pipe::None,
            });
        } else {
            let pipe = pipe_kind(text(child, source));
            pending_input = pipe;
            if let Some(last) = stages.last_mut() {
                last.output = pipe;
            }
        }
    }
    if let Some(last) = stages.last_mut() {
        last.redirects.extend(trailing);
    }
    stages
}

fn pipe_kind(token: &str) -> Pipe {
    match token {
        PIPE => Pipe::Stdout,
        PIPE_WITH_STDERR => Pipe::StdoutAndStderr,
        _ => Pipe::None,
    }
}

fn split_stage(stage: Node<'_>) -> (Node<'_>, Vec<Node<'_>>) {
    if stage.kind() == REDIRECTED {
        let mut redirects = field_children(stage, REDIRECT_FIELD);
        let Some(body) = stage.child_by_field_name(BODY_FIELD) else {
            return (stage, redirects);
        };
        redirects.extend(field_children(body, REDIRECT_FIELD));
        return (body, redirects);
    }
    (stage, field_children(stage, REDIRECT_FIELD))
}

fn check_stage(stage: &Stage<'_>, source: &[u8], findings: &mut Findings) {
    let command = Command::of(stage.body);
    let basename = command
        .as_ref()
        .and_then(|command| command.name(source))
        .map(|name| name.rsplit(PATH_SEPARATOR).next().unwrap_or_default().to_owned());
    let non_reading = basename
        .as_deref()
        .filter(|basename| NON_READING_COMMANDS.contains(basename));
    let stage_consumes = contains_consumer(stage.body)
        || stage
            .redirects
            .iter()
            .any(|redirect| contains_consumer(*redirect));
    if let (Some(command), Some(non_reading_name)) = (&command, non_reading) {
        let reads_stage = NonReading {
            name: non_reading_name,
            piped_without_consumer: stage.input != Pipe::None && !stage_consumes,
            redirects_stdin: stage
                .redirects
                .iter()
                .any(|redirect| descriptors_of(*redirect, source).is_some_and(|list| list.contains(&STDIN))),
        };
        check_non_reading(command, &reads_stage, source, findings);
    }
    let Some(descriptors) = descriptor_map(&stage.redirects, source) else {
        return;
    };
    if stage.input != Pipe::None
        && !stage_consumes
        && let Some(override_redirect) = descriptors.get(&STDIN).and_then(|list| list.first())
    {
        findings.report_at(
            ShellRule::Sc2259,
            operator_position(*override_redirect, source),
            "",
        );
    }
    let produces = contains_producer(stage.body)
        || stage
            .redirects
            .iter()
            .any(|redirect| contains_producer(*redirect));
    if stage.output == Pipe::Stdout
        && !produces
        && let Some(override_redirect) = descriptors.get(&STDOUT).and_then(|list| list.first())
    {
        findings.report_at(
            ShellRule::Sc2260,
            operator_position(*override_redirect, source),
            "",
        );
    }
    for competing in descriptors.values().filter(|list| list.len() > 1) {
        for redirect in competing {
            findings.report_at(ShellRule::Sc2261, operator_position(*redirect, source), "");
        }
    }
}

struct NonReading<'name> {
    name: &'name str,
    piped_without_consumer: bool,
    redirects_stdin: bool,
}

fn check_non_reading(command: &Command<'_>, stage: &NonReading<'_>, source: &[u8], findings: &mut Findings) {
    let name = stage.name;
    let is_interactive = INTERACTIVE_COMMANDS.contains(&name)
        && INTERACTIVE_FLAGS
            .iter()
            .any(|flag| command.has_flag(flag, source));
    let reads_list = name == DU
        && command
            .flags(source)
            .iter()
            .any(|(_, flag)| DU_READING_FLAGS.contains(&flag.as_str()));
    if stage.piped_without_consumer && !is_interactive && !reads_list {
        findings.report(ShellRule::Sc2216, command.node, name);
    }
    if stage.redirects_stdin && !contains_consumer(command.node) && !is_interactive {
        findings.report(ShellRule::Sc2217, command.node, name);
    }
}

fn contains_consumer(node: Node<'_>) -> bool {
    descendants(node).into_iter().any(|inner| {
        inner.kind() == COMMAND_SUBSTITUTION
            || (inner.kind() == PROCESS_SUBSTITUTION && first_token_is(inner, OPEN_INPUT_SUBSTITUTION))
    })
}

fn contains_producer(node: Node<'_>) -> bool {
    descendants(node)
        .into_iter()
        .any(|inner| inner.kind() == PROCESS_SUBSTITUTION && first_token_is(inner, OPEN_OUTPUT_SUBSTITUTION))
}

fn first_token_is(node: Node<'_>, expected: &str) -> bool {
    node.child(0).is_some_and(|first| first.kind() == expected)
}

// `Map.fromListWith (++)`: en cada descriptor, la última redirección queda primera. Si una no se puede
// entender, ShellCheck no mira ninguna (`mapM getRedirectionFds`).
fn descriptor_map<'tree>(
    redirects: &[Node<'tree>],
    source: &[u8],
) -> Option<BTreeMap<u32, Vec<Node<'tree>>>> {
    let mut descriptors: BTreeMap<u32, Vec<Node<'tree>>> = BTreeMap::new();
    for redirect in redirects {
        for descriptor in descriptors_of(*redirect, source)? {
            descriptors.entry(descriptor).or_default().insert(0, *redirect);
        }
    }
    Some(descriptors)
}

fn descriptors_of(redirect: Node<'_>, source: &[u8]) -> Option<Vec<u32>> {
    let defaults = default_descriptors(redirect, source)?;
    redirect
        .child_by_field_name(DESCRIPTOR_FIELD)
        .map_or(Some(defaults), |descriptor| {
            text(descriptor, source)
                .parse::<u32>()
                .ok()
                .map(|number| vec![number])
        })
}

fn default_descriptors(redirect: Node<'_>, source: &[u8]) -> Option<Vec<u32>> {
    match redirect.kind() {
        HEREDOC_REDIRECT | HERESTRING => Some(vec![STDIN]),
        FILE_REDIRECT => {
            let operator = operator_of(redirect, source);
            let destination = redirect.child_by_field_name(DESTINATION_FIELD);
            let target = destination.map_or("", |destination| text(destination, source));
            match operator {
                READ => Some(vec![STDIN]),
                WRITE | APPEND | CLOBBER => Some(vec![STDOUT]),
                WRITE_BOTH | APPEND_BOTH => Some(vec![STDOUT, STDERR]),
                DUPLICATE_OUTPUT if target == CLOSE_DESCRIPTOR || !is_descriptor_number(target) => {
                    Some(vec![STDOUT, STDERR])
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_descriptor_number(target: &str) -> bool {
    !target.is_empty() && target.bytes().all(|byte| byte.is_ascii_digit())
}

fn operator_of<'source>(redirect: Node<'_>, source: &'source [u8]) -> &'source str {
    children(redirect)
        .into_iter()
        .find(|child| !child.is_named())
        .map_or("", |operator| text(operator, source))
}

// `getOpId`: el operador, sin el `&` de `&>`, que ShellCheck lee como el descriptor.
fn operator_position(redirect: Node<'_>, source: &[u8]) -> Position {
    if redirect.kind() == HEREDOC_REDIRECT
        && let Some(start) = named_children(redirect)
            .into_iter()
            .find(|child| child.kind() == HEREDOC_START)
    {
        return Position::of(start);
    }
    let Some(operator) = children(redirect).into_iter().find(|child| !child.is_named()) else {
        return Position::of(redirect);
    };
    let skipped = usize::from(text(operator, source).starts_with(BOTH_DESCRIPTORS_MARK));
    Position::of(operator).shifted(skipped)
}
