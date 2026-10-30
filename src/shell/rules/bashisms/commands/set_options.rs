use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::super::Found;
use crate::rules::ShellRule;
use crate::shell::words::literal;

const OPTION_LETTERS: &str = "abCefhmnuvxo";
const LONG_OPTIONS: &[&str] = &[
    "allexport",
    "errexit",
    "ignoreeof",
    "monitor",
    "noclobber",
    "noexec",
    "noglob",
    "nolog",
    "notify",
    "nounset",
    "verbose",
    "vi",
    "xtrace",
];

static STARTS_OPTION: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^(\+|-[^-])").ok());
static TAKES_OPTION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(&format!("^[-+][{OPTION_LETTERS}]*o$")).ok());
static VALID_FLAGS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(&format!("^[-+]([{OPTION_LETTERS}]+o?|o)$")).ok());
static DOUBLE_DASH: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new("^--.+$").ok());

fn matches(regex: &LazyLock<Option<Regex>>, text: &str) -> bool {
    regex.as_ref().is_some_and(|regex| regex.is_match(text))
}

// `set` de `checkBashisms`: las opciones literales hasta la primera que no lo es, con `-o nombre`
// contra las opciones largas de POSIX y cada letra contra las cortas.
pub fn check(arguments: &[Node<'_>], source: &[u8], found: &mut Found<'_>) {
    let literals: Vec<(Node<'_>, String)> = arguments
        .iter()
        .map_while(|argument| literal(*argument, source).map(|text| (*argument, text)))
        .collect();
    check_options(&literals, found);
}

fn check_options(arguments: &[(Node<'_>, String)], found: &mut Found<'_>) {
    match arguments {
        [flag, option, rest @ ..] if matches(&TAKES_OPTION, &flag.1) => {
            if !LONG_OPTIONS.contains(&option.1.as_str()) {
                found.report(ShellRule::Sc3040, option.0, &option.1);
            }
            let mut remaining = vec![flag.clone()];
            remaining.extend_from_slice(rest);
            check_flags(&remaining, found);
        }
        [] => {}
        _ => check_flags(arguments, found),
    }
}

fn check_flags(arguments: &[(Node<'_>, String)], found: &mut Found<'_>) {
    let Some(((word, flag), rest)) = arguments.split_first() else {
        return;
    };
    if matches(&STARTS_OPTION, flag) {
        if !matches(&VALID_FLAGS, flag) {
            for letter in flag
                .chars()
                .skip(1)
                .filter(|letter| !OPTION_LETTERS.contains(*letter))
            {
                found.report(ShellRule::Sc3041, *word, &format!("-{letter}"));
            }
        }
        check_options(rest, found);
    } else if matches(&DOUBLE_DASH, flag) {
        found.report(ShellRule::Sc3042, *word, flag);
        check_options(rest, found);
    }
}
