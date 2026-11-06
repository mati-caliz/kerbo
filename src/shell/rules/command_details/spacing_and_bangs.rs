use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{
    ASSIGNMENT, COMMAND, NAME_FIELD, NEGATED, PIPELINE, REDIRECT_FIELD, REDIRECTED, SUBSHELL, VALUE_FIELD,
    children, field_children, named_children, text,
};
use crate::shell::words::oversimplify;

const ECHO: &str = "echo";
const TIME: &str = "time";
const OPEN_BRACE: &str = "{";
const BANG: &str = "!";
const PROMPT_VARIABLE: &str = "PS1";
const ESCAPE_SEQUENCES: &[&str] = &["\\x1B", "\\x1b", "\\e", "\u{1b}", "\\033"];
const ENCLOSED_START: &str = "\\[";
const ENCLOSED_END: &str = "\\]";
const MIN_EXTRA_SPACING: usize = 4;
const PATH_SEPARATOR: char = '/';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let is_sh_family = matches!(context.dialect, Dialect::Sh | Dialect::Dash | Dialect::Busybox);
    match node.kind() {
        COMMAND => {
            check_echo_spacing(node, source, findings);
            if is_sh_family {
                check_timed_compound(node, source, findings);
            }
        }
        PIPELINE => {
            if is_sh_family {
                check_timed_pipeline(node, source, findings);
            }
            if context.dialect != Dialect::Ksh {
                check_bang_after_pipe(node, findings);
            }
        }
        NEGATED if is_sh_family => check_multiple_bangs(node, source, findings),
        ASSIGNMENT if context.dialect == Dialect::Bash => check_prompt(node, source, findings),
        _ => {}
    }
}

// SC2291: `checkUnquotedEchoSpaces`.
fn check_echo_spacing(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let is_echo = command.name(source).is_some_and(|name| {
        name.rsplit(PATH_SEPARATOR)
            .next()
            .is_some_and(|basename| basename == ECHO)
    });
    if !is_echo {
        return;
    }
    let redirects: Vec<usize> = node
        .parent()
        .filter(|parent| parent.kind() == REDIRECTED)
        .map_or_else(Vec::new, |parent| field_children(parent, REDIRECT_FIELD))
        .iter()
        .map(Node::start_byte)
        .collect();
    let has_gap = command.arguments().windows(2).any(|pair| {
        let [left, right] = pair else {
            return false;
        };
        let same_line = left.start_position().row == right.end_position().row;
        let gap = right
            .start_position()
            .column
            .saturating_sub(left.end_position().column);
        same_line
            && gap >= MIN_EXTRA_SPACING
            && !redirects
                .iter()
                .any(|redirect| left.end_byte() < *redirect && *redirect < right.start_byte())
    });
    if has_gap {
        findings.report(ShellRule::Sc2291, node, ECHO);
    }
}

// SC2176: `checkTimedCommand`, `time a | b`.
fn check_timed_pipeline(pipeline: Node<'_>, source: &[u8], findings: &mut Findings) {
    let first = named_children(pipeline).first().copied();
    let name_word = first
        .and_then(Command::of)
        .filter(|command| command.is_exactly(TIME, source) && !command.arguments().is_empty())
        .and_then(|command| command.first_word());
    if let Some(word) = name_word {
        findings.report(ShellRule::Sc2176, word, TIME);
    }
}

// SC2177: `checkTimedCommand`, `time { a; }` y `time ( a )`.
fn check_timed_compound(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(command) = Command::of(node).filter(|command| command.is_exactly(TIME, source)) else {
        return;
    };
    let compound = named_children(node)
        .into_iter()
        .find(|child| child.kind() == SUBSHELL)
        .or_else(|| {
            command
                .arguments()
                .first()
                .copied()
                .filter(|argument| text(*argument, source) == OPEN_BRACE)
        });
    if let Some(compound) = compound {
        findings.report(ShellRule::Sc2177, compound, TIME);
    }
}

// SC2325: `checkMultipleBangs`.
fn check_multiple_bangs(negated: Node<'_>, source: &[u8], findings: &mut Findings) {
    let bangs = children(negated)
        .iter()
        .filter(|child| !child.is_named() && text(**child, source) == BANG)
        .count();
    let nested = named_children(negated).first().is_some_and(|inner| {
        inner.kind() == NEGATED
            || Command::of(*inner)
                .is_some_and(|command| command.literal_name(source).as_deref() == Some(BANG))
    });
    if bangs > 1 || nested {
        findings.report(ShellRule::Sc2325, negated, BANG);
    }
}

// SC2326: `checkBangAfterPipe`.
fn check_bang_after_pipe(pipeline: Node<'_>, findings: &mut Findings) {
    for stage in named_children(pipeline).into_iter().skip(1) {
        if stage.kind() == NEGATED {
            findings.report(ShellRule::Sc2326, stage, BANG);
        }
    }
}

// SC2025: `checkPS1Assignments`.
fn check_prompt(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_prompt = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == PROMPT_VARIABLE);
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD).filter(|_| is_prompt) else {
        return;
    };
    let contents = oversimplify(value, source);
    let unenclosed = match (contents.find(ENCLOSED_START), contents.rfind(ENCLOSED_END)) {
        (Some(start), Some(end)) if start < end => {
            let before = contents.get(..start).unwrap_or_default();
            let after = contents
                .get(end.saturating_add(ENCLOSED_END.len())..)
                .unwrap_or_default();
            format!("{before}{after}")
        }
        _ => contents,
    };
    if ESCAPE_SEQUENCES
        .iter()
        .any(|sequence| unenclosed.contains(sequence))
    {
        findings.report(ShellRule::Sc2025, value, PROMPT_VARIABLE);
    }
}
