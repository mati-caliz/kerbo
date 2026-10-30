use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::Invocation;
use super::word_shapes::{is_glob, is_literal_part, single_unmodified_expansion, word_parts};
use crate::rules::ShellRule;
use crate::shell::commands::{gnu_options, is_flag};
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::parameters::{is_array_expansion, is_variable_name, modifier};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, COMMAND_SUBSTITUTION, EXPANSION, EXPANSIONS, FUNCTION, NUMBER, RAW_STRING,
    SIMPLE_EXPANSION, STRING_CONTENT, WORD, ancestors, text,
};
use crate::shell::words::{braced_content, literal, literal_or, oversimplify, parts};

const RETURN: &str = "return";
const EXIT: &str = "exit";
const TRAP: &str = "trap";
const SET: &str = "set";
const EXPORT: &str = "export";
const READ: &str = "read";
const ALIAS: &str = "alias";
const UNSET: &str = "unset";
const LOCAL: &str = "local";
const LET: &str = "let";
const EVAL: &str = "eval";
const DOT: &str = ".";
const TIME: &str = "time";
const READ_OPTIONS: &str = "sreu:n:N:i:p:a:t:";
const ARRAY_FLAG: &str = "a";
const EXPANDED_STATUS: &str = "0";
const UNKNOWN_STATUS: &str = "WTF";
const MAXIMUM_STATUS: u32 = 255;
const MAXIMUM_STATUS_DIGITS: usize = 5;
const EXIT_TRAP: &str = "0";
const PORTABLE_SIGNAL_NUMBERS: &[&str] = &["1", "2", "3", "6", "9", "14", "15"];
const UNTRAPPABLE: &[&str] = &["kill", "9", "sigkill", "stop", "sigstop"];
const NOT_LITERAL: &str = "*";
const ASSIGNMENT: char = '=';
const POSITIONAL_PARAMETER: &str = r"\$\{?[0-9*@]";
const NOT_LITERAL_ALIAS: &str = "_";
const QUOTING_MODIFIER: char = 'Q';
const OPEN_CLASS: char = '[';
const CLOSE_CLASS: char = ']';
const POSIX_TIME_FLAG: &str = "-p";
const FLAG_PREFIX: &str = "-";
const EXPANDED_KINDS: &[&str] = &[
    SIMPLE_EXPANSION,
    EXPANSION,
    COMMAND_SUBSTITUTION,
    ARITHMETIC_EXPANSION,
];

static POSITIONAL: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(POSITIONAL_PARAMETER).ok());

pub fn check(invocation: &Invocation<'_, '_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let arguments = invocation.arguments();
    if invocation.is_exactly(RETURN) {
        check_status(
            arguments,
            source,
            (ShellRule::Sc2151, ShellRule::Sc2152),
            findings,
        );
    }
    if invocation.is_exactly(EXIT) {
        check_status(
            arguments,
            source,
            (ShellRule::Sc2241, ShellRule::Sc2242),
            findings,
        );
    }
    if invocation.is_exactly(TRAP) {
        check_signals(arguments, source, findings);
    }
    if invocation.is_exactly(SET) {
        check_set_assignment(arguments, source, findings);
    }
    if invocation.is_exactly(EXPORT) {
        check_expansions_as_names(arguments, source, ShellRule::Sc2163, findings);
    }
    if invocation.is_exactly(READ) {
        check_read(arguments, source, findings);
    }
    if invocation.is_exactly(ALIAS) {
        check_alias(arguments, source, findings);
    }
    if invocation.is_exactly(UNSET) {
        check_unset_globs(arguments, source, findings);
    }
    check_shell_specific(invocation, context, findings);
    if invocation.is_exactly(EVAL) {
        check_eval(arguments, source, findings);
    }
}

fn check_shell_specific(invocation: &Invocation<'_, '_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let dialect = context.dialect;
    let source = context.source;
    let in_function = ancestors(invocation.node()).any(|ancestor| ancestor.kind() == FUNCTION);
    if invocation.is_exactly(LOCAL)
        && !in_function
        && matches!(dialect, Dialect::Bash | Dialect::Dash | Dialect::Busybox)
    {
        findings.report(ShellRule::Sc2168, invocation.name_word(), LOCAL);
    }
    if invocation.is_exactly(LET) && matches!(dialect, Dialect::Bash | Dialect::Ksh) {
        findings.report(ShellRule::Sc2219, invocation.node(), LET);
    }
    if invocation.is_exactly(DOT)
        && matches!(dialect, Dialect::Sh | Dialect::Dash)
        && let Some(argument) = invocation.arguments().get(1)
    {
        findings.report(ShellRule::Sc2240, *argument, text(*argument, source));
    }
    if invocation.is_exactly(TIME)
        && matches!(dialect, Dialect::Bash | Dialect::Sh)
        && let Some(argument) = invocation.arguments().first()
    {
        let flag = oversimplify(*argument, source);
        if flag.starts_with(FLAG_PREFIX) && flag != POSIX_TIME_FLAG {
            findings.report(ShellRule::Sc2023, invocation.name_word(), TIME);
        }
    }
}

// `return` y `exit` aceptan un solo número de 0 a 255; una expansión cuenta como un número válido.
fn check_status(
    arguments: &[Node<'_>],
    source: &[u8],
    (several, invalid): (ShellRule, ShellRule),
    findings: &mut Findings,
) {
    match arguments {
        [first, _, ..] => findings.report(several, *first, text(*first, source)),
        [value] if is_invalid_status(&status_text(*value, source)) => {
            findings.report(invalid, *value, text(*value, source));
        }
        _ => {}
    }
}

fn status_text(word: Node<'_>, source: &[u8]) -> String {
    parts(word)
        .into_iter()
        .map(|part| match part.kind() {
            kind if EXPANDED_KINDS.contains(&kind) => EXPANDED_STATUS.to_owned(),
            WORD | NUMBER | STRING_CONTENT | RAW_STRING => {
                literal(part, source).unwrap_or_else(|| UNKNOWN_STATUS.to_owned())
            }
            _ => UNKNOWN_STATUS.to_owned(),
        })
        .collect()
}

fn is_invalid_status(status: &str) -> bool {
    status.is_empty()
        || !status.chars().all(|digit| digit.is_ascii_digit())
        || status.len() > MAXIMUM_STATUS_DIGITS
        || status.parse::<u32>().is_ok_and(|number| number > MAXIMUM_STATUS)
}

fn check_signals(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let Some((first, signals)) = arguments.split_first() else {
        return;
    };
    if is_flag(*first, source) {
        return;
    }
    for signal in signals {
        let Some(name) = literal(*signal, source) else {
            continue;
        };
        let numeric = !name.is_empty() && name.chars().all(|digit| digit.is_ascii_digit());
        if numeric && name != EXIT_TRAP && !PORTABLE_SIGNAL_NUMBERS.contains(&name.as_str()) {
            findings.report(ShellRule::Sc2172, *signal, &name);
        }
        if UNTRAPPABLE.contains(&name.to_lowercase().as_str()) {
            findings.report(ShellRule::Sc2173, *signal, &name);
        }
    }
}

fn check_set_assignment(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let Some((variable, rest)) = arguments.split_first() else {
        return;
    };
    let name: String = parts(*variable)
        .into_iter()
        .map(|part| {
            if part.kind() == WORD {
                literal(part, source).unwrap_or_else(|| NOT_LITERAL.to_owned())
            } else {
                NOT_LITERAL.to_owned()
            }
        })
        .collect();
    if (!rest.is_empty() && is_variable_name(&name)) || name.contains(ASSIGNMENT) {
        findings.report(ShellRule::Sc2121, *variable, &name);
    }
}

fn check_expansions_as_names(
    arguments: &[Node<'_>],
    source: &[u8],
    rule: ShellRule,
    findings: &mut Findings,
) {
    for argument in arguments {
        if let Some(name) = single_unmodified_expansion(*argument, source) {
            findings.report(rule, *argument, &name);
        }
    }
}

fn check_read(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let variables: Vec<Node<'_>> = gnu_options(READ_OPTIONS, arguments, source)
        .unwrap_or_default()
        .into_iter()
        .filter(|(flag, _)| flag.is_empty() || flag == ARRAY_FLAG)
        .map(|(_, option)| option.value)
        .collect();
    for variable in variables {
        if let Some(name) =
            single_unmodified_expansion(variable, source).filter(|name| is_variable_name(name))
        {
            findings.report(ShellRule::Sc2229, variable, &name);
        }
    }
    for argument in arguments {
        let unquoted: String = parts(*argument)
            .into_iter()
            .filter(|part| part.kind() == WORD)
            .map(|part| text(part, source))
            .collect();
        if unquoted
            .split_once(OPEN_CLASS)
            .is_some_and(|(_, after)| after.contains(CLOSE_CLASS))
        {
            findings.report(ShellRule::Sc2313, *argument, text(*argument, source));
        }
    }
}

fn check_alias(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    for argument in arguments {
        let definition = literal_or(*argument, source, NOT_LITERAL_ALIAS);
        let uses_parameters = POSITIONAL
            .as_ref()
            .is_some_and(|regex| regex.is_match(&definition));
        if definition.contains(ASSIGNMENT) && uses_parameters {
            findings.report(ShellRule::Sc2142, *argument, &definition);
        }
        if oversimplify(*argument, source).contains(ASSIGNMENT)
            && let Some(part) = word_parts(*argument)
                .into_iter()
                .find(|part| !is_literal_part(*part, source))
        {
            findings.report(ShellRule::Sc2139, part, text(part, source));
        }
    }
}

// tree-sitter-bash parte `unset foo[1]` en el nombre y `[1]` pegado: para ShellCheck es una sola
// palabra con un glob.
fn check_unset_globs(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let mut previous: Option<Node<'_>> = None;
    for argument in arguments {
        let glued = previous.is_some_and(|before| before.end_byte() == argument.start_byte());
        let word = if glued { previous } else { Some(*argument) };
        if is_glob(*argument, source)
            && let Some(word) = word
        {
            findings.report(ShellRule::Sc2184, word, text(word, source));
        }
        previous = Some(*argument);
    }
}

fn check_eval(arguments: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    for part in arguments.iter().flat_map(|argument| word_parts(*argument)) {
        if !EXPANSIONS.contains(&part.kind()) {
            continue;
        }
        let braced = braced_content(part, source);
        if !is_array_expansion(&braced) {
            continue;
        }
        if part.kind() == EXPANSION && modifier(&braced).contains(QUOTING_MODIFIER) {
            findings.report(ShellRule::Sc2293, part, &braced);
        } else {
            findings.report(ShellRule::Sc2294, part, &braced);
        }
    }
}
