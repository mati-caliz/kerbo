use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::Found;
use crate::rules::ShellRule;
use crate::shell::context::Context;
use crate::shell::parameters::{is_variable_character, reference};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, EXPANSION, EXPANSIONS, NAME_FIELD, SUBSCRIPT, VARIABLE_NAME, WORD,
    ancestors, is_arithmetic_command, text,
};
use crate::shell::words::braced_content;

const BASH_VARIABLES: &[&str] = &[
    "OSTYPE",
    "MACHTYPE",
    "HOSTTYPE",
    "HOSTNAME",
    "DIRSTACK",
    "EUID",
    "UID",
    "SHLVL",
    "PIPESTATUS",
    "SHELLOPTS",
    "_",
    "BASHOPTS",
    "BASHPID",
    "BASH_ALIASES",
    "BASH_ARGC",
    "BASH_ARGV",
    "BASH_ARGV0",
    "BASH_CMDS",
    "BASH_COMMAND",
    "BASH_EXECUTION_STRING",
    "BASH_LINENO",
    "BASH_REMATCH",
    "BASH_SOURCE",
    "BASH_SUBSHELL",
    "BASH_VERSINFO",
    "EPOCHREALTIME",
    "EPOCHSECONDS",
    "FUNCNAME",
    "GROUPS",
    "MAPFILE",
];
const DYNAMIC_VARIABLES: &[&str] = &["RANDOM", "SECONDS"];
const DASH_VARIABLES: &[&str] = &["_"];
const ARITHMETIC_CONTEXTS: &[&str] = &[ARITHMETIC_EXPANSION, ARITHMETIC_FOR];

// Las expresiones de `checkBashisms` sobre lo que hay entre `${` y `}`: las simples no valen para
// busybox, que sí las tiene.
const SIMPLE_EXPANSIONS: &[(&str, ShellRule)] = &[
    (r"^[_0-9a-zA-Z*@]+:[^-=?+]", ShellRule::Sc3057),
    (r"^([*@][%#]|#[@*])", ShellRule::Sc3058),
    (r"^[_0-9a-zA-Z*@]+(\[.*\])?/", ShellRule::Sc3060),
];
const ADVANCED_EXPANSIONS: &[(&str, ShellRule)] = &[
    (r"^![_0-9a-zA-Z]", ShellRule::Sc3053),
    (r"^[_0-9a-zA-Z]+\[.*\]$", ShellRule::Sc3054),
    (r"^![_0-9a-zA-Z]+\[[*@]\]$", ShellRule::Sc3055),
    (r"^![_0-9a-zA-Z]+[*@]$", ShellRule::Sc3056),
    (r"^[_0-9a-zA-Z*@]+(\[.*\])?[,^]", ShellRule::Sc3059),
];

type Patterns = Vec<(Regex, ShellRule)>;

static SIMPLE: LazyLock<Patterns> = LazyLock::new(|| compile(SIMPLE_EXPANSIONS));
static ADVANCED: LazyLock<Patterns> = LazyLock::new(|| compile(ADVANCED_EXPANSIONS));

fn compile(patterns: &[(&str, ShellRule)]) -> Patterns {
    patterns
        .iter()
        .filter_map(|(pattern, rule)| Regex::new(pattern).ok().map(|regex| (regex, *rule)))
        .collect()
}

pub fn check(node: Node<'_>, context: &Context<'_, '_>, assigned: &HashSet<String>, found: &mut Found<'_>) {
    let source = context.source;
    if EXPANSIONS.contains(&node.kind()) {
        let braced = braced_content(node, source);
        if !found.shell().is_busybox() {
            report_matches(&SIMPLE, node, &braced, found);
        }
        report_matches(&ADVANCED, node, &braced, found);
        check_variable(node, &reference(&braced), assigned, found);
    } else if is_arithmetic_variable(node, source) {
        check_variable(node, text(node, source), assigned, found);
    }
}

fn report_matches(patterns: &Patterns, node: Node<'_>, braced: &str, found: &mut Found<'_>) {
    for (regex, rule) in patterns {
        if regex.is_match(braced) {
            found.report(*rule, node, braced);
        }
    }
}

fn check_variable(node: Node<'_>, name: &str, assigned: &HashSet<String>, found: &mut Found<'_>) {
    let bash_only =
        DYNAMIC_VARIABLES.contains(&name) || (BASH_VARIABLES.contains(&name) && !assigned.contains(name));
    if bash_only && !(found.shell().is_dash() && DASH_VARIABLES.contains(&name)) {
        found.report(ShellRule::Sc3028, node, name);
    }
}

// Un nombre leído como variable dentro de una expresión aritmética (`TA_Variable` de ShellCheck).
fn is_arithmetic_variable(node: Node<'_>, source: &[u8]) -> bool {
    if node.kind() != VARIABLE_NAME && node.kind() != WORD {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    let is_subscript_name =
        parent.kind() == SUBSCRIPT && parent.child_by_field_name(NAME_FIELD) == Some(node);
    if EXPANSIONS.contains(&parent.kind()) || is_subscript_name || parent.kind() == EXPANSION {
        return false;
    }
    let is_identifier = text(node, source).chars().all(is_variable_character);
    is_identifier
        && ancestors(node)
            .take_while(|ancestor| !EXPANSIONS.contains(&ancestor.kind()))
            .any(|ancestor| {
                ARITHMETIC_CONTEXTS.contains(&ancestor.kind()) || is_arithmetic_command(ancestor, source)
            })
}
