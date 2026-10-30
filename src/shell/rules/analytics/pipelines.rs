use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::{Command, flags_of};
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{BODY_FIELD, PIPELINE, REDIRECTED, named_children, text};
use crate::shell::words::oversimplify;

const FIND: &str = "find";
const XARGS: &str = "xargs";
const PS: &str = "ps";
const GREP: &str = "grep";
const WC: &str = "wc";
const LS: &str = "ls";
const OPTION_PREFIX: &str = "-";
const OPTION_DASH: char = '-';
const NULL_SEPARATED_FLAG: char = '0';
const NULL_SEPARATED_OPTIONS: &[&str] = &["null", "print0", "printf"];
const PID_FLAGS: &[&str] = &["p", "pid", "q", "quick-pid"];
const GREP_COUNTING_FLAGS: &[&str] = &[
    "l",
    "files-with-matches",
    "L",
    "files-without-matches",
    "o",
    "only-matching",
    "r",
    "R",
    "recursive",
    "A",
    "after-context",
    "B",
    "before-context",
];
const WC_NOT_LINES_FLAGS: &[&str] = &["m", "chars", "w", "words", "c", "bytes", "L", "max-line-length"];
const NUMERIC_SORT_FLAG: char = 'N';

// Un comando del pipeline con sus palabras como las simplifica ShellCheck; lo que no es un comando
// simple no tiene nombre.
struct Stage<'tree> {
    node: Node<'tree>,
    command: Option<Command<'tree>>,
    words: Vec<String>,
}

impl Stage<'_> {
    fn name(&self) -> &str {
        self.words.first().map_or("", String::as_str)
    }

    fn flags(&self, source: &[u8]) -> Vec<String> {
        self.command
            .as_ref()
            .map(|command| {
                flags_of(command.arguments(), source)
                    .into_iter()
                    .map(|(_, flag)| flag)
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn stage<'tree>(node: Node<'tree>, source: &[u8]) -> Stage<'tree> {
    let simple = if node.kind() == REDIRECTED {
        node.child_by_field_name(BODY_FIELD)
    } else {
        Some(node)
    };
    let command = simple.and_then(Command::of);
    let words = command
        .as_ref()
        .map(|command| {
            command
                .words
                .iter()
                .map(|word| oversimplify(*word, source))
                .collect()
        })
        .unwrap_or_default();
    Stage { node, command, words }
}

// `checkPipePitfalls` de ShellCheck: pares de comandos seguidos de un pipeline que tienen una forma
// mejor.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != PIPELINE {
        return;
    }
    let source = context.source;
    let stages: Vec<Stage<'_>> = named_children(node)
        .into_iter()
        .map(|child| stage(child, source))
        .collect();
    for pair in stages.windows(2) {
        let [first, second] = pair else {
            continue;
        };
        match (first.name(), second.name()) {
            (FIND, XARGS) if !separates_with_null(first, second) => {
                findings.report(ShellRule::Sc2038, first.node, FIND);
            }
            (PS, GREP)
                if !first
                    .flags(source)
                    .iter()
                    .any(|flag| PID_FLAGS.contains(&flag.as_str())) =>
            {
                findings.report(ShellRule::Sc2009, first.node, PS);
            }
            (GREP, WC) if counts_lines(first, second, source) => {
                findings.report(ShellRule::Sc2126, first.node, GREP);
            }
            _ => {}
        }
    }
    check_listings(&stages, source, findings);
}

fn separates_with_null(find: &Stage<'_>, xargs: &Stage<'_>) -> bool {
    let words: Vec<&String> = xargs.words.iter().chain(find.words.iter()).collect();
    words
        .iter()
        .any(|word| word.starts_with(OPTION_PREFIX) && word.contains(NULL_SEPARATED_FLAG))
        || words.iter().any(|word| {
            let option = word.trim_start_matches(OPTION_DASH);
            NULL_SEPARATED_OPTIONS.iter().any(|name| option.starts_with(name))
        })
}

fn counts_lines(grep: &Stage<'_>, wc: &Stage<'_>, source: &[u8]) -> bool {
    let wc_flags = wc.flags(source);
    !grep
        .flags(source)
        .iter()
        .any(|flag| GREP_COUNTING_FLAGS.contains(&flag.as_str()))
        && !wc_flags
            .iter()
            .any(|flag| WC_NOT_LINES_FLAGS.contains(&flag.as_str()))
        && !wc_flags.is_empty()
}

// `ls | grep` y `ls | xargs` tienen su aviso; si no hay ninguno, cualquier `ls` que alimenta un
// pipeline avisa SC2012.
fn check_listings(stages: &[Stage<'_>], source: &[u8], findings: &mut Findings) {
    let mut listed = false;
    for pair in stages.windows(2) {
        let [ls, next] = pair else {
            continue;
        };
        let rule = match (ls.name(), next.name()) {
            (LS, GREP) => ShellRule::Sc2010,
            (LS, XARGS) => ShellRule::Sc2011,
            _ => continue,
        };
        listed = true;
        let name_word = ls.command.as_ref().and_then(|command| command.name_word(source));
        findings.report(rule, name_word.unwrap_or(ls.node), LS);
    }
    if listed {
        return;
    }
    for pair in stages.windows(2) {
        let [ls, _] = pair else {
            continue;
        };
        let numeric = ls
            .words
            .iter()
            .any(|word| word.starts_with(OPTION_PREFIX) && word.contains(NUMERIC_SORT_FLAG));
        if ls.name() == LS && !numeric {
            findings.report(ShellRule::Sc2012, ls.node, text(ls.node, source));
        }
    }
}
