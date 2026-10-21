use tree_sitter::Node;

use super::expansions::is_just_command_output;
use crate::fix::Fix;
use crate::rules::ShellRule;
use crate::shell::commands::{Command, gnu_options};
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::quoting::is_condition;
use crate::shell::syntax::{
    ASSIGNMENT, BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, EXPANSIONS, FUNCTION, HEREDOC_REDIRECT,
    REDIRECTED, ancestors, children, field_children, is_backticked, named_children, text,
};
use crate::shell::words::{literal, oversimplify, parts};

const READ: &str = "read";
const READ_OPTIONS: &str = "sreu:n:N:i:p:a:t:";
const RAW_FLAG: &str = "r";
const TIMEOUT_FLAG: &str = "t";
const ZERO_TIMEOUT: &str = "0";
const RAW_INSERTION: &str = " -r";
const DIRECTORY_COMMANDS: &[&str] = &["cd", "pushd", "popd"];
const STACK_COMMANDS: &[&str] = &["pushd", "popd"];
const NO_CHANGE_FLAG: &str = "n";
const EXIT_INSERTION: &str = " || exit";
const TRAP: &str = "trap";
const PRINTF: &str = "printf";
const END_OF_OPTIONS: &str = "--";
const VARIABLE_FLAG: &str = "-v";
const FORMAT_MARKER: char = '%';
const ECHO: &str = "echo";
const BUILTIN: &str = "builtin";
const DECLARING: &[&str] = &["local", "declare", "export", "readonly", "typeset", "let"];
const SCOPED_DECLARING: &[&str] = &["local", "declare", "typeset"];
const READONLY: &str = "readonly";
const READONLY_FLAG: &str = "r";
const GLOBAL_FLAG: &str = "g";
const FUNCTION_KEYWORD: &str = "function";
const PARENT_DIRECTORY: &str = "..";
const CURRENT_DIRECTORY: &str = ".";
const PATH_SEPARATOR: char = '/';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    check_read(&command, context, findings);
    check_directory_change(&command, context, findings);
    let Some(name) = exact_name(&command, context.source) else {
        return;
    };
    let arguments = exact_arguments(&command, context.source);
    match name.as_str() {
        TRAP => check_trap(arguments, context, findings),
        PRINTF => check_printf(arguments, context, findings),
        ECHO => check_echo(arguments, context, findings),
        _ => {}
    }
    if DECLARING.contains(&name.as_str()) {
        check_masked_returns(&command, &name, context, findings);
    }
}

fn exact_name(command: &Command<'_>, source: &[u8]) -> Option<String> {
    let name = command.literal_name(source)?;
    if name != BUILTIN {
        return Some(name);
    }
    command
        .arguments()
        .first()
        .and_then(|word| literal(*word, source))
}

fn exact_arguments<'command, 'tree>(
    command: &'command Command<'tree>,
    source: &[u8],
) -> &'command [Node<'tree>] {
    let arguments = command.arguments();
    if command.literal_name(source).as_deref() == Some(BUILTIN) {
        arguments.get(1..).unwrap_or_default()
    } else {
        arguments
    }
}

fn check_read(command: &Command<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if !command.is_exactly(READ, source) || command.has_flag(RAW_FLAG, source) {
        return;
    }
    let zero_timeout = gnu_options(READ_OPTIONS, command.arguments(), source).is_some_and(|options| {
        options
            .iter()
            .find(|(flag, _)| flag == TIMEOUT_FLAG)
            .is_some_and(|(_, option)| literal(option.value, source).as_deref() == Some(ZERO_TIMEOUT))
    });
    if zero_timeout {
        return;
    }
    let Some(word) = command.name_word(source) else {
        return;
    };
    let fix = Fix::insert_at(word.end_position(), RAW_INSERTION, source);
    findings.report_fixed(ShellRule::Sc2162, word, (text(word, source), fix));
}

fn check_directory_change(command: &Command<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if context.has_set_e || command.node.kind() != COMMAND {
        return;
    }
    let Some(name) = command.name(source) else {
        return;
    };
    if !DIRECTORY_COMMANDS.contains(&name.as_str())
        || is_safe_directory(command, source)
        || (STACK_COMMANDS.contains(&name.as_str()) && command.has_flag(NO_CHANGE_FLAG, source))
        || is_condition(command.node, source)
    {
        return;
    }
    let end = insertion_end(command.node);
    let fix = Fix::insert_at(end.end_position(), EXIT_INSERTION, source);
    findings.report_fixed(ShellRule::Sc2164, command.node, (&name, fix));
}

fn is_safe_directory(command: &Command<'_>, source: &[u8]) -> bool {
    let [_, argument] = command.words.as_slice() else {
        return false;
    };
    let path = oversimplify(*argument, source);
    path.trim_start_matches(PATH_SEPARATOR)
        .split(PATH_SEPARATOR)
        .filter(|segment| !segment.is_empty())
        .all(|segment| segment == CURRENT_DIRECTORY || segment == PARENT_DIRECTORY)
}

fn insertion_end(command: Node<'_>) -> Node<'_> {
    command
        .parent()
        .filter(|parent| {
            parent.kind() == REDIRECTED
                && parent.child_by_field_name(BODY_FIELD) == Some(command)
                && !children(*parent)
                    .iter()
                    .any(|child| child.kind() == HEREDOC_REDIRECT)
        })
        .unwrap_or(command)
}

fn check_trap(arguments: &[Node<'_>], context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(first) = arguments.first() else {
        return;
    };
    if first.kind() != crate::shell::syntax::STRING {
        return;
    }
    for part in named_children(*first) {
        let expands = EXPANSIONS.contains(&part.kind())
            || part.kind() == COMMAND_SUBSTITUTION
            || part.kind() == crate::shell::syntax::ARITHMETIC_EXPANSION;
        if expands {
            findings.report(ShellRule::Sc2064, part, context.text(part));
        }
    }
}

fn check_printf(arguments: &[Node<'_>], context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let mut remaining = arguments;
    loop {
        match remaining {
            [first, rest @ ..] if literal(*first, source).as_deref() == Some(END_OF_OPTIONS) => {
                remaining = rest;
            }
            [first, _, rest @ ..] if literal(*first, source).as_deref() == Some(VARIABLE_FLAG) => {
                remaining = rest;
            }
            _ => break,
        }
    }
    let Some(format) = remaining.first() else {
        return;
    };
    if !oversimplify(*format, source).contains(FORMAT_MARKER) && literal(*format, source).is_none() {
        findings.report(ShellRule::Sc2059, *format, context.text(*format));
    }
}

fn check_echo(arguments: &[Node<'_>], context: &Context<'_, '_>, findings: &mut Findings) {
    if let [only] = arguments
        && is_just_command_output(*only, context.source)
    {
        findings.report(ShellRule::Sc2005, *only, context.text(*only));
    }
}

fn check_masked_returns(
    command: &Command<'_>,
    name: &str,
    context: &Context<'_, '_>,
    findings: &mut Findings,
) {
    let source = context.source;
    let has_global = command.has_flag(GLOBAL_FLAG, source);
    let is_local = !has_global
        && SCOPED_DECLARING.contains(&name)
        && ancestors(command.node).any(|ancestor| is_scoped_function(ancestor, context));
    let is_read_only = name == READONLY || command.has_flag(READONLY_FLAG, source);
    if is_local && is_read_only {
        return;
    }
    for argument in exact_arguments(command, source) {
        if argument.kind() != ASSIGNMENT {
            continue;
        }
        let masks = field_children(*argument, crate::shell::syntax::VALUE_FIELD)
            .into_iter()
            .flat_map(parts)
            .any(|part| part.kind() == COMMAND_SUBSTITUTION || is_backticked(part, source));
        if masks {
            findings.report(ShellRule::Sc2155, *argument, context.text(*argument));
        }
    }
}

fn is_scoped_function(node: Node<'_>, context: &Context<'_, '_>) -> bool {
    node.kind() == FUNCTION
        && (context.dialect != Dialect::Ksh
            || children(node)
                .first()
                .is_some_and(|first| text(*first, context.source) == FUNCTION_KEYWORD))
}
