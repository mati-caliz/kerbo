use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    ASSIGNMENT, NAME_FIELD, OPERATOR_FIELD, RAW_STRING, UNARY, ancestors, is_statement, text,
};
use crate::shell::words::literal;

const SINGLE_QUOTE: char = '\'';
const ESCAPE: u8 = b'\\';
const DOLLAR: char = '$';
const BACKTICK: char = '`';
const EXPANSION_STARTS: &[char] = &['{', '('];
const SED: &str = "sed";
const SED_COMMANDS: &[char] = &['{', 'd', 'p', 's', 'a', 'i', 'c'];
const FIND: &str = "find";
const FIND_EXEC: &[&str] = &["-exec", "-execdir"];
const GIT: &str = "git";
const FILTER_BRANCH: &str = "filter-branch";
const GIT_FILTER_BRANCH: &str = "git filter-branch";
const MUMPS: &str = "mumps";
const MUMPS_RUN: &str = "-run";
const MUMPS_COMMANDS: &[&str] = &["%XCMD", "LOOP%XCMD"];
const AWK_SUFFIX: &str = "awk";
const PERL_PREFIX: &str = "perl";
const TEST_VARIABLE: &str = "-v";
const PATH_SEPARATOR: char = '/';
const ANCESTORS_CHECKED: usize = 2;
const COMMONLY_QUOTED: &[&str] = &["PS1", "PS2", "PS3", "PS4", "PROMPT_COMMAND"];
const EVALUATING_COMMANDS: &[&str] = &[
    "trap",
    "sh",
    "bash",
    "ksh",
    "zsh",
    "ssh",
    "eval",
    "xprop",
    "alias",
    "sudo",
    "doas",
    "run0",
    "docker",
    "podman",
    "oc",
    "dpkg-query",
    "jq",
    "rename",
    "rg",
    "unset",
    "git filter-branch",
    "mumps -run %XCMD",
    "mumps -run LOOP%XCMD",
];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let escaped = node
        .start_byte()
        .checked_sub(1)
        .and_then(|before| context.source.get(before))
        .is_some_and(|before| *before == ESCAPE);
    if node.kind() != RAW_STRING || escaped {
        return;
    }
    let quoted = context.text(node).trim_matches(SINGLE_QUOTE);
    if !looks_like_expansion(quoted) {
        return;
    }
    let command_name = closest_command_name(node, context.source);
    let quiet = if command_name == SED {
        has_sed_dollar(quoted)
    } else {
        is_probably_ok(node, &command_name, context.source)
    };
    if !quiet {
        findings.report(ShellRule::Sc2016, node, context.text(node));
    }
}

fn looks_like_expansion(content: &str) -> bool {
    let characters: Vec<char> = content.chars().collect();
    let dollar = characters.windows(2).any(|pair| {
        pair.first() == Some(&DOLLAR)
            && pair.get(1).is_some_and(|next| {
                EXPANSION_STARTS.contains(next) || next.is_ascii_alphanumeric() || *next == '_'
            })
    });
    let backticks = content
        .split(BACKTICK)
        .skip(1)
        .collect::<Vec<_>>()
        .split_last()
        .is_some_and(|(_, inner)| inner.iter().any(|between| !between.is_empty()));
    dollar || backticks
}

fn has_sed_dollar(content: &str) -> bool {
    let characters: Vec<char> = content.chars().collect();
    characters.iter().enumerate().any(|(index, character)| {
        *character == DOLLAR
            && characters
                .get(index.saturating_add(1))
                .is_some_and(|next| SED_COMMANDS.contains(next))
            && characters
                .get(index.saturating_add(2))
                .is_none_or(|after| !after.is_ascii_alphabetic())
    })
}

fn is_probably_ok(node: Node<'_>, command_name: &str, source: &[u8]) -> bool {
    let assignment_or_test = std::iter::once(node)
        .chain(ancestors(node))
        .take(ANCESTORS_CHECKED.saturating_add(1))
        .any(|ancestor| is_ok_assignment(ancestor, source));
    assignment_or_test
        || EVALUATING_COMMANDS.contains(&command_name)
        || command_name.ends_with(AWK_SUFFIX)
        || command_name.starts_with(PERL_PREFIX)
}

fn is_ok_assignment(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        ASSIGNMENT => node
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| COMMONLY_QUOTED.contains(&text(name, source))),
        UNARY => node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == TEST_VARIABLE),
        _ => false,
    }
}

fn closest_command_name(node: Node<'_>, source: &[u8]) -> String {
    let Some(statement) = ancestors(node).find(|ancestor| is_statement(*ancestor)) else {
        return String::new();
    };
    let Some(command) = Command::of(statement) else {
        return String::new();
    };
    let Some(name) = command.name(source) else {
        return String::new();
    };
    let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or(&name).to_owned();
    match basename.as_str() {
        FIND => find_command(&command, source),
        GIT => git_command(&command, source),
        MUMPS => mumps_command(&command, source),
        _ => basename,
    }
}

fn words(command: &Command<'_>, source: &[u8]) -> Vec<Option<String>> {
    command.words.iter().map(|word| literal(*word, source)).collect()
}

fn find_command(command: &Command<'_>, source: &[u8]) -> String {
    let literals = words(command, source);
    let mut after_exec = literals
        .iter()
        .skip_while(|word| !word.as_deref().is_some_and(|word| FIND_EXEC.contains(&word)));
    match (after_exec.next(), after_exec.next()) {
        (Some(_), Some(Some(executed))) => executed.clone(),
        _ => FIND.to_owned(),
    }
}

fn git_command(command: &Command<'_>, source: &[u8]) -> String {
    match words(command, source).as_slice() {
        [Some(git), Some(subcommand), ..] if git == GIT && subcommand == FILTER_BRANCH => {
            GIT_FILTER_BRANCH.to_owned()
        }
        _ => GIT.to_owned(),
    }
}

fn mumps_command(command: &Command<'_>, source: &[u8]) -> String {
    match words(command, source).as_slice() {
        [Some(mumps), Some(run), Some(target), ..]
            if mumps == MUMPS && run == MUMPS_RUN && MUMPS_COMMANDS.contains(&target.as_str()) =>
        {
            format!("{MUMPS} {MUMPS_RUN} {target}")
        }
        _ => MUMPS.to_owned(),
    }
}
