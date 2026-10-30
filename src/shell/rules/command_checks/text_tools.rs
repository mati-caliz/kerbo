use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::Invocation;
use super::word_shapes::{is_glob, literal_with_globs, will_split};
use crate::rules::ShellRule;
use crate::shell::commands::flags_of;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{WORD, text};
use crate::shell::words::{literal, literal_or, oversimplify};

const TR: &str = "tr";
const GREP: &str = "grep";
const EXPR: &str = "expr";
const LOWERCASE_RANGE: &str = "a-z";
const UPPERCASE_RANGE: &str = "A-Z";
const FLAG_PREFIX: &str = "-";
const CLASS_PREFIX: &str = "[:";
const EQUIVALENCE_PREFIX: &str = "[=";
const OPEN_BRACKET: &str = "[";
const CLOSE_BRACKET: &str = "]";
const STAR: char = '*';
const MINIMUM_BRACKETED: usize = 2;
const NOT_LITERAL: &str = "_";
const PATTERN_FLAGS: &[&str] = &["--", "-e", "--regex"];
const REGEX_ASSIGNMENT: &str = "--regex=";
const GLOB_FLAGS: &[&str] = &[
    "fixed-strings",
    "F",
    "include",
    "exclude",
    "exclude-dir",
    "o",
    "only-matching",
];
const GLOB_STAR: &str = "*";
const REGEX_ESCAPES: &[char] = &['\\', '.'];
const PAIR: usize = 2;
const EXPR_EXCEPTIONS: &[&str] = &[":", "<", ">", "<=", ">=", "match", "length", "substr", "index"];
const UNSPECIFIED_OPERATIONS: &[&str] = &["match", "length", "substr", "index"];
const MATCH_OPERATOR: &str = ":";
const LENGTH: &str = "length";
const SUSPICIOUS_WILDCARD: &str = r"([A-Za-z1-9])\*";
const HARMLESS_WILDCARD: &str = r"[^a-zA-Z1-9]\*|[\]\[^$+\\]";

static SUSPICIOUS: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(SUSPICIOUS_WILDCARD).ok());
static HARMLESS: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(HARMLESS_WILDCARD).ok());

pub fn check(invocation: &Invocation<'_, '_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if invocation.has_basename(TR) {
        for argument in invocation.arguments() {
            check_tr_argument(*argument, source, findings);
        }
    }
    if invocation.has_basename(GREP) {
        check_grep(invocation, source, findings);
    }
    if invocation.has_basename(EXPR) {
        check_expr(invocation, source, findings);
    }
}

fn check_tr_argument(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    if is_glob(word, source) {
        findings.report(ShellRule::Sc2060, word, text(word, source));
        return;
    }
    let Some(set) = literal(word, source) else {
        return;
    };
    match set.as_str() {
        LOWERCASE_RANGE => findings.report(ShellRule::Sc2018, word, &set),
        UPPERCASE_RANGE => findings.report(ShellRule::Sc2019, word, &set),
        _ => {
            if !set.starts_with(FLAG_PREFIX) && !set.contains(CLASS_PREFIX) && has_repeated_letters(&set) {
                findings.report(ShellRule::Sc2020, word, &set);
            }
            let class = set.starts_with(CLASS_PREFIX) || set.starts_with(EQUIVALENCE_PREFIX);
            if !class
                && set.starts_with(OPEN_BRACKET)
                && set.ends_with(CLOSE_BRACKET)
                && set.chars().count() > MINIMUM_BRACKETED
                && !set.contains(STAR)
            {
                findings.report(ShellRule::Sc2021, word, &set);
            }
        }
    }
}

fn has_repeated_letters(set: &str) -> bool {
    let mut seen = HashSet::new();
    set.chars()
        .filter(|letter| letter.is_alphabetic())
        .any(|letter| !seen.insert(letter))
}

// El patrón de grep es el primer argumento que no es opción, o el que sigue a `-e`, `--regex` o `--`.
fn check_grep(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let arguments = invocation.arguments();
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        let text = literal_with_globs(*argument, source, NOT_LITERAL);
        if PATTERN_FLAGS.contains(&text.as_str()) {
            check_pattern(
                invocation,
                arguments.get(index.saturating_add(1)).copied(),
                source,
                findings,
            );
            return;
        }
        if text.starts_with(REGEX_ASSIGNMENT) || !text.starts_with(FLAG_PREFIX) {
            check_pattern(invocation, Some(*argument), source, findings);
            return;
        }
        index = index.saturating_add(1);
    }
}

fn check_pattern(
    invocation: &Invocation<'_, '_>,
    pattern: Option<Node<'_>>,
    source: &[u8],
    findings: &mut Findings,
) {
    let Some(pattern) = pattern else {
        return;
    };
    if is_glob(pattern, source) {
        findings.report(ShellRule::Sc2062, pattern, text(pattern, source));
    }
    let matches_globs = flags_of(invocation.arguments(), source)
        .iter()
        .any(|(_, flag)| GLOB_FLAGS.contains(&flag.as_str()));
    if matches_globs {
        return;
    }
    let regex = oversimplify(pattern, source);
    if looks_like_glob(&regex) {
        findings.report(ShellRule::Sc2063, pattern, &regex);
    } else if let Some(wildcard) = suspicious_wildcard(&regex) {
        findings.report(ShellRule::Sc2022, pattern, &wildcard);
    }
}

// `isConfusedGlobRegex`: `*foo` o `x*`, que en una regex no significan lo que en un glob.
fn looks_like_glob(regex: &str) -> bool {
    let characters: Vec<char> = regex.chars().collect();
    regex.starts_with(GLOB_STAR)
        || (characters.len() == PAIR
            && characters.get(1) == Some(&STAR)
            && characters
                .first()
                .is_some_and(|first| !REGEX_ESCAPES.contains(first)))
}

fn suspicious_wildcard(regex: &str) -> Option<String> {
    let suspicious = SUSPICIOUS.as_ref()?.captures(regex)?.get(1)?.as_str().to_owned();
    let harmless = HARMLESS.as_ref().is_some_and(|harmless| harmless.is_match(regex));
    (!harmless).then_some(suspicious)
}

fn check_expr(invocation: &Invocation<'_, '_>, source: &[u8], findings: &mut Findings) {
    let arguments = invocation.arguments();
    let literals: Vec<String> = arguments
        .iter()
        .filter_map(|word| literal(*word, source))
        .collect();
    if literals
        .iter()
        .all(|word| !EXPR_EXCEPTIONS.contains(&word.as_str()))
    {
        findings.report(ShellRule::Sc2003, invocation.name_word(), EXPR);
    }
    match arguments {
        [left, operator, right] => {
            check_operation(*left, source, findings);
            if operator.kind() == WORD && text(*operator, source) == GLOB_STAR {
                findings.report(ShellRule::Sc2304, *operator, GLOB_STAR);
            } else if operator.kind() == WORD
                && text(*operator, source) == MATCH_OPERATOR
                && is_glob(*right, source)
            {
                findings.report(ShellRule::Sc2305, *right, text(*right, source));
            }
        }
        [single] if !will_split(*single, source) => findings.report(ShellRule::Sc2307, *single, EXPR),
        [first, second]
            if literal_or(*first, source, "") != LENGTH
                && !will_split(*first, source)
                && !will_split(*second, source) =>
        {
            check_operation(*first, source, findings);
            findings.report(ShellRule::Sc2307, invocation.node(), EXPR);
        }
        [first, rest @ ..] => {
            check_operation(*first, source, findings);
            for argument in rest.iter().filter(|argument| is_glob(**argument, source)) {
                findings.report(ShellRule::Sc2306, *argument, text(*argument, source));
            }
        }
        [] => {}
    }
}

fn check_operation(side: Node<'_>, source: &[u8], findings: &mut Findings) {
    if let Some(operation) =
        literal(side, source).filter(|word| UNSPECIFIED_OPERATIONS.contains(&word.as_str()))
    {
        findings.report(ShellRule::Sc2308, side, &operation);
    }
}
