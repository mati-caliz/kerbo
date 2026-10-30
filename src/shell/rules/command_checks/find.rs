use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::Invocation;
use super::word_shapes::is_glob;
use crate::rules::ShellRule;
use crate::shell::commands::flags_of;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{DESTINATION_FIELD, FILE_REDIRECT, REDIRECTED, children, field_children, text};
use crate::shell::words::{literal, literal_or};

const FIND: &str = "find";
const GLOB_ARGUMENTS: &[&str] = &[
    "-ilname",
    "-iname",
    "-ipath",
    "-iregex",
    "-iwholename",
    "-lname",
    "-name",
    "-path",
    "-regex",
    "-wholename",
];
const EXEC_ACTIONS: &[&str] = &["-exec", "-execdir"];
const EXEC_TERMINATORS: &[&str] = &[";", "+"];
const SHELLS: &[&str] = &["sh", "bash", "dash", "ksh"];
const SHELL_COMMAND_FLAG: &str = "-c";
const FILE_PLACEHOLDER: &str = "{}";
const MISSING_COMMAND: &str = " ";
const COMMAND_SEPARATORS: &str = "[ |;]";
const MATCH_TESTS: &[&str] = &[
    "-name",
    "-regex",
    "-iname",
    "-iregex",
    "-wholename",
    "-iwholename",
];
const OR_OPERATORS: &[&str] = &["-o", "-or"];
const ACTIONS: &[&str] = &[
    "-exec", "-execdir", "-delete", "-print", "-print0", "-fls", "-fprint", "-fprint0", "-fprintf", "-ls",
    "-ok", "-okdir", "-printf",
];
const PRECEDENCE_WINDOW: usize = 6;
const HELP_FLAG: &str = "help";
const NOT_LITERAL_FLAG: &str = "___";
const FLAG_PREFIX: &str = "-";
const LEADING_FLAG_CHARACTERS: &str = "-EHLPXdfsxO0123456789";
const SHORT_FLAG_LENGTH: usize = 2;

static SEPARATORS: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(COMMAND_SEPARATORS).ok());

pub fn check(invocation: &Invocation<'_, '_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if !invocation.has_basename(FIND) {
        return;
    }
    let source = context.source;
    let arguments = invocation.arguments();
    check_name_globs(arguments, source, findings);
    check_exec_without_shell(arguments, source, findings);
    check_injected_file_names(arguments, source, findings);
    check_action_precedence(arguments, source, findings);
    check_missing_path(invocation, source, findings);
    check_redirections(invocation, source, findings);
}

fn literal_in(word: Node<'_>, source: &[u8], options: &[&str]) -> bool {
    literal(word, source).is_some_and(|text| options.contains(&text.as_str()))
}

fn check_name_globs(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    for pair in arguments.windows(2) {
        if let [test, pattern] = pair
            && literal_in(*test, source, GLOB_ARGUMENTS)
            && is_glob(*pattern, source)
        {
            findings.report(ShellRule::Sc2061, *pattern, &literal_or(*test, source, ""));
        }
    }
}

fn check_exec_without_shell(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    for triple in arguments.windows(3) {
        let [exec, command, terminator] = triple else {
            continue;
        };
        let command_text = literal_or(*command, source, MISSING_COMMAND);
        let has_separator = SEPARATORS
            .as_ref()
            .is_some_and(|regex| regex.is_match(&command_text));
        if literal_in(*exec, source, EXEC_ACTIONS)
            && literal_in(*terminator, source, EXEC_TERMINATORS)
            && has_separator
        {
            findings.report(ShellRule::Sc2150, *exec, &command_text);
        }
    }
}

// `-exec sh -c '... {} ...'`: el `{}` dentro del script de la shell, en cualquier orden con lo del
// medio, como el `match` de ShellCheck.
fn check_injected_file_names(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let texts: Vec<String> = arguments
        .iter()
        .map(|word| literal_or(*word, source, ""))
        .collect();
    let positions = |options: &[&str], from: usize| -> Vec<usize> {
        texts
            .iter()
            .enumerate()
            .skip(from)
            .filter(|(_, text)| options.contains(&text.as_str()))
            .map(|(index, _)| index)
            .collect()
    };
    let mut reported = Vec::new();
    for exec in positions(EXEC_ACTIONS, 0) {
        for shell in positions(SHELLS, exec.saturating_add(1)) {
            let scripts = positions(&[SHELL_COMMAND_FLAG], shell.saturating_add(1))
                .into_iter()
                .map(|flag| flag.saturating_add(1))
                .filter(|script| {
                    texts
                        .get(*script)
                        .is_some_and(|text| text.contains(FILE_PLACEHOLDER))
                });
            reported.extend(scripts);
        }
    }
    reported.sort_unstable();
    reported.dedup();
    for script in reported {
        if let Some(word) = arguments.get(script) {
            findings.report(ShellRule::Sc2156, *word, FILE_PLACEHOLDER);
        }
    }
}

fn check_action_precedence(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    for window in arguments.windows(PRECEDENCE_WINDOW) {
        let [first, _, or, second, _, action] = window else {
            continue;
        };
        if literal_in(*first, source, MATCH_TESTS)
            && literal_in(*or, source, OR_OPERATORS)
            && literal_in(*second, source, MATCH_TESTS)
            && literal_in(*action, source, ACTIONS)
        {
            findings.report(ShellRule::Sc2146, *action, &literal_or(*action, source, ""));
            return;
        }
    }
}

fn check_missing_path(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let asks_help = flags_of(invocation.arguments(), source)
        .iter()
        .any(|(_, flag)| flag == HELP_FLAG);
    if !asks_help && !has_path(invocation.arguments(), source) {
        findings.report(ShellRule::Sc2185, invocation.name_word(), FIND);
    }
}

// Las opciones de antes de la ruta son de una letra de GNU o macOS; lo primero que no empieza con
// `-` es la ruta.
fn has_path(arguments: &[Node<'_>], source: &[u8]) -> bool {
    let Some((first, rest)) = arguments.split_first() else {
        return false;
    };
    let flag = literal(*first, source).unwrap_or_else(|| NOT_LITERAL_FLAG.to_owned());
    let leading = flag.chars().count() <= SHORT_FLAG_LENGTH
        || flag
            .chars()
            .all(|letter| LEADING_FLAG_CHARACTERS.contains(letter));
    !flag.starts_with(FLAG_PREFIX) || (leading && has_path(rest, source))
}

// tree-sitter-bash pone lo que sigue a una redirección como destinos de más de ella: si una tiene
// más de uno, la redirección está antes de algún argumento.
fn check_redirections(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let node = invocation.node();
    if invocation.arguments().is_empty() {
        return;
    }
    let Some(statement) = node.parent().filter(|parent| parent.kind() == REDIRECTED) else {
        return;
    };
    let redirects: Vec<Node<'_>> = children(statement)
        .into_iter()
        .filter(|child| child.kind() == FILE_REDIRECT)
        .collect();
    let in_the_middle = redirects
        .iter()
        .any(|redirect| field_children(*redirect, DESTINATION_FIELD).len() > 1);
    if in_the_middle && let Some(first) = redirects.first() {
        findings.report(ShellRule::Sc2227, *first, text(*first, source));
    }
}
