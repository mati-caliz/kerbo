use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::Invocation;
use super::word_shapes::may_become_multiple_args;
use crate::rules::ShellRule;
use crate::shell::commands::{bsd_options, flags_of};
use crate::shell::context::{Context, Findings, Position};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{
    DESCRIPTOR_FIELD, DESTINATION_FIELD, EXPANSION, FILE_REDIRECT, PIPELINE, REDIRECTED, STRING,
    STRING_CONTENT, WORD, ancestors, children, named_children, text,
};
use crate::shell::words::{braced_content, literal, literal_or, oversimplify, parts};

mod removal;

const ECHO: &str = "echo";
const MKDIR: &str = "mkdir";
const SU: &str = "su";
const SSH: &str = "ssh";
const SUDO: &str = "sudo";
const CHMOD: &str = "chmod";
const XARGS: &str = "xargs";
const DEPRECATED: &[(&str, ShellRule)] = &[
    ("tempfile", ShellRule::Sc2186),
    ("egrep", ShellRule::Sc2196),
    ("fgrep", ShellRule::Sc2197),
];
const MISSING_DESTINATION: &[(&str, ShellRule)] = &[
    ("mv", ShellRule::Sc2224),
    ("cp", ShellRule::Sc2225),
    ("ln", ShellRule::Sc2226),
];
const TARGET_DIRECTORY: &str = "target-directory";
const ESCAPES_FLAG: &str = "e";
const ECHO_ESCAPES: &str = r"\\([rntabefv\\']|[0-7]{1,3}|x([0-9]|[A-F]|[a-f]){1,2})";
const PARENTS_FLAGS: &[&str] = &["p", "parents"];
const MODE_FLAGS: &[&str] = &["m", "mode"];
const PATH_SEPARATOR: char = '/';
const SIBLING_DIRECTORY: &str = r"^(\.\.?/)+[^/]+$";
const OPTION_PREFIX: &str = "-";
const SUDO_OPTIONS: &str = "vAknSbEHPa:g:h:p:u:c:T:r:";
const SUDO_BUILTINS: &[&str] = &[
    "cd", "command", "declare", "eval", "exec", "exit", "export", "hash", "history", "local", "popd",
    "pushd", "read", "readonly", "return", "set", "source", "trap", "type", "typeset", "ulimit", "umask",
    "unset", "wait",
];
const NULL_DEVICE: &str = "/dev/null";
const BOTH_STREAMS: &str = "&";
const SUDO_REDIRECTS: &[&str] = &["<", ">", ">>", "&>", "&>>"];
const RECURSIVE_READ: &str = "-r";
const XARGS_OPTIONS: &str = "0oprtxadR:S:J:L:l:n:P:s:e:E:i:I:";
const REPLACE_FLAG: &str = "i";
const INTERACTIVE_ARGUMENTS: usize = 1;
const GUARDED: &[&str] = &[":?", ":-", ":="];

static ESCAPES: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(ECHO_ESCAPES).ok());
static SIBLING: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(SIBLING_DIRECTORY).ok());

pub fn check(invocation: &Invocation<'_, '_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if invocation.has_basename(ECHO) && matches!(context.dialect, Dialect::Sh | Dialect::Bash | Dialect::Ksh)
    {
        check_echo_escapes(invocation, source, findings);
    }
    if invocation.has_basename(MKDIR) {
        check_mkdir_mode(invocation, source, findings);
    }
    if invocation.has_basename(SU) {
        check_interactive_su(invocation, findings);
    }
    if invocation.has_basename(SSH) {
        check_ssh(invocation.arguments(), source, findings);
    }
    for (name, rule) in DEPRECATED {
        if invocation.has_basename(name) {
            findings.report(*rule, invocation.name_word(), name);
        }
    }
    removal::check(invocation, source, findings);
    for (name, rule) in MISSING_DESTINATION {
        if invocation.has_basename(name) {
            check_destination(invocation, source, *rule, findings);
        }
    }
    if invocation.has_basename(SUDO) {
        check_sudo_redirects(invocation, source, findings);
        check_sudo_builtin(invocation, source, findings);
    }
    if invocation.has_basename(CHMOD) {
        for argument in invocation.arguments() {
            if literal(*argument, source).as_deref() == Some(RECURSIVE_READ) {
                findings.report(ShellRule::Sc2253, *argument, RECURSIVE_READ);
            }
        }
    }
    if invocation.has_basename(XARGS)
        && let Some((_, option)) = bsd_options(XARGS_OPTIONS, invocation.arguments(), source)
            .unwrap_or_default()
            .into_iter()
            .find(|(flag, _)| flag == REPLACE_FLAG)
    {
        findings.report(ShellRule::Sc2267, option.flag, REPLACE_FLAG);
    }
}

fn check_echo_escapes(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    if flags_of(invocation.arguments(), source)
        .iter()
        .any(|(_, flag)| flag == ESCAPES_FLAG)
    {
        return;
    }
    for argument in invocation.arguments() {
        let content = literal_or(*argument, source, "");
        if ESCAPES.as_ref().is_some_and(|regex| regex.is_match(&content)) {
            findings.report(ShellRule::Sc2028, *argument, &content);
        }
    }
}

fn check_mkdir_mode(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let flags = flags_of(invocation.arguments(), source);
    let has_parents = flags
        .iter()
        .any(|(_, flag)| PARENTS_FLAGS.contains(&flag.as_str()));
    let Some((mode, _)) = flags.iter().find(|(_, flag)| MODE_FLAGS.contains(&flag.as_str())) else {
        return;
    };
    let nested = invocation.arguments().iter().skip(1).any(|argument| {
        literal(*argument, source).is_none_or(|name| {
            name.contains(PATH_SEPARATOR) && !SIBLING.as_ref().is_some_and(|regex| regex.is_match(&name))
        })
    });
    if has_parents && nested {
        findings.report(ShellRule::Sc2174, *mode, text(*mode, source));
    }
}

// `su` sin comando y sin nada que le dé la entrada: abre una shell interactiva en medio del script.
fn check_interactive_su(invocation: &Invocation<'_, '_>, findings: &mut Findings) {
    let node = invocation.node();
    let redirected = |candidate: Node<'_>| {
        children(candidate)
            .iter()
            .any(|child| child.kind() == FILE_REDIRECT || child.kind() == "herestring_redirect")
    };
    let directed = std::iter::once(node).chain(ancestors(node)).any(|ancestor| {
        (ancestor.kind() == PIPELINE && named_children(ancestor).len() > 1)
            || (ancestor.kind() == REDIRECTED && redirected(ancestor))
    });
    if invocation.arguments().len() <= INTERACTIVE_ARGUMENTS && !directed {
        findings.report(ShellRule::Sc2117, node, SU);
    }
}

fn check_ssh(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let is_option = |word: &Node<'_>| oversimplify(*word, source).starts_with(OPTION_PREFIX);
    if arguments.iter().any(is_option) || arguments.len() < 2 {
        return;
    }
    let Some(remote) = arguments.last().filter(|remote| remote.kind() == STRING) else {
        return;
    };
    if let Some(expansion) = named_children(*remote)
        .into_iter()
        .find(|part| part.kind() != STRING_CONTENT)
    {
        findings.report(ShellRule::Sc2029, expansion, text(expansion, source));
    }
}

fn check_destination(
    invocation: &Invocation<'_, '_>,
    source: &[u8],
    rule: ShellRule,
    findings: &mut Findings,
) {
    let flags = flags_of(invocation.arguments(), source);
    let parameters: Vec<Node<'_>> = flags
        .iter()
        .filter(|(_, flag)| flag.is_empty())
        .map(|(word, _)| *word)
        .collect();
    let has_target = flags
        .iter()
        .any(|(_, flag)| !flag.is_empty() && TARGET_DIRECTORY.starts_with(flag.as_str()));
    if let [single] = parameters.as_slice()
        && !has_target
        && !may_become_multiple_args(*single, source)
    {
        findings.report(rule, invocation.node(), text(*single, source));
    }
}

fn check_sudo_redirects(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let node = invocation.node();
    let mut redirects: Vec<Node<'_>> = children(node)
        .into_iter()
        .filter(|child| child.kind() == FILE_REDIRECT)
        .collect();
    if let Some(statement) = node.parent().filter(|parent| parent.kind() == REDIRECTED) {
        redirects.extend(
            children(statement)
                .into_iter()
                .filter(|child| child.kind() == FILE_REDIRECT),
        );
    }
    for redirect in redirects {
        let descriptor = redirect.child_by_field_name(DESCRIPTOR_FIELD);
        if descriptor.is_some_and(|descriptor| text(descriptor, source) != BOTH_STREAMS) {
            continue;
        }
        let Some(operator) = children(redirect).into_iter().find(|child| !child.is_named()) else {
            continue;
        };
        let operator_text = text(operator, source);
        let file = redirect
            .child_by_field_name(DESTINATION_FIELD)
            .map(|destination| oversimplify(destination, source))
            .unwrap_or_default();
        if SUDO_REDIRECTS.contains(&operator_text) && file != NULL_DEVICE {
            let shift = usize::from(operator_text.starts_with(BOTH_STREAMS));
            findings.report_at(
                ShellRule::Sc2024,
                Position::of(operator).shifted(shift),
                operator_text,
            );
        }
    }
}

fn check_sudo_builtin(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let Some(options) = bsd_options(SUDO_OPTIONS, invocation.arguments(), source) else {
        return;
    };
    let command = options
        .into_iter()
        .find(|(flag, _)| flag.is_empty())
        .and_then(|(_, option)| literal(option.value, source));
    if let Some(command) = command.filter(|command| SUDO_BUILTINS.contains(&command.as_str())) {
        findings.report(ShellRule::Sc2232, invocation.node(), &command);
    }
}

// Lo que `getPotentialPath` de ShellCheck saca de una palabra para `rm`: los globs como texto, las
// expansiones vacías y `None` si alguna tiene un valor por defecto o un error con `:`.
pub fn potential_path(word: Node<'_>, source: &[u8]) -> Option<String> {
    let mut path = String::new();
    for part in parts(word) {
        match part.kind() {
            EXPANSION
                if GUARDED
                    .iter()
                    .any(|guard| braced_content(part, source).contains(guard)) =>
            {
                return None;
            }
            WORD => path.push_str(text(part, source)),
            _ => path.push_str(&literal(part, source).unwrap_or_default()),
        }
    }
    Some(path)
}
