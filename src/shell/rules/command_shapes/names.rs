use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::parameters::is_variable_name;
use crate::shell::syntax::{
    ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, COMMAND, COMPOUND, DECLARATION, FUNCTION, LIST, NAME_FIELD,
    PIPELINE, REDIRECTED, STRING, STRING_CONTENT, SUBSHELL, is_arithmetic_command, named_children, text,
};
use crate::shell::variables::{EventKind, flow};
use crate::shell::word_pieces::{has_glob, only_literal_text, pieces};
use crate::shell::words::literal;

const FLAG_PREFIX: char = '-';
const ANY_TEXT: &str = "x";
const TRANSLATION_MARK: &str = "$";
const SUBSTITUTION_STARTS: &[char] = &['(', '{'];
const TRAILING_SYMBOLS: &[char] = &[
    '\\', '.', ',', '(', '[', '{', '<', '>', '}', ']', ')', '#', '"', '\'', '%', ' ',
];
const ALLOWED_NAMES: &[&str] = &[".", ":", " ", "//"];
const ESCAPED_DOT: &str = r"\.";
const SLASH: char = '/';
const TAB: char = '\t';
const LINE_FEED: char = '\n';
const VARIABLE_EXTRA: char = '_';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    match node.kind() {
        COMMAND => {
            if let Some(command) = Command::of(node) {
                check_command_name(&command, source, findings);
            }
        }
        ASSIGNMENT => check_flag_assignment(node, source, findings),
        FUNCTION => check_recursion(node, source, findings),
        STRING => check_translated_string(node, context, findings),
        _ => {}
    }
}

// SC2211, SC2215 y SC2286 a SC2289: `checkGlobAsCommand`, `checkFlagAsCommand` y
// `checkCommandWithTrailingSymbol`.
fn check_command_name(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    let Some(first) = command.first_word() else {
        return;
    };
    let word_pieces = pieces(first, source);
    let subject = text(first, source);
    if has_glob(&word_pieces) {
        findings.report(ShellRule::Sc2211, first, subject);
    }
    let leading_flag = word_pieces.first().is_some_and(|piece| {
        piece.kind == crate::shell::word_pieces::PieceKind::Literal && piece.text.starts_with(FLAG_PREFIX)
    });
    if command.assignments.is_empty() && leading_flag {
        findings.report(ShellRule::Sc2215, first, subject);
    }
    let name = only_literal_text(&word_pieces, ANY_TEXT);
    if ALLOWED_NAMES.contains(&name.as_str()) || command.is_misread_group(source) || subject == ESCAPED_DOT {
        return;
    }
    let rule = match name.chars().last() {
        None => Some(ShellRule::Sc2286),
        Some(SLASH) => Some(ShellRule::Sc2287),
        Some(last) if TRAILING_SYMBOLS.contains(&last) => Some(ShellRule::Sc2288),
        Some(_) if name.contains(TAB) || name.contains(LINE_FEED) => Some(ShellRule::Sc2289),
        Some(_) => None,
    };
    if let Some(rule) = rule {
        findings.report(rule, first, subject);
    }
}

// tree-sitter lee `--bar=baz` como una asignación a una variable llamada `--bar`.
fn check_flag_assignment(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_flag = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source).starts_with(FLAG_PREFIX));
    let is_alone = assignment
        .parent()
        .is_none_or(|parent| ![COMMAND, DECLARATION, ASSIGNMENTS].contains(&parent.kind()));
    if is_flag && is_alone {
        findings.report(ShellRule::Sc2215, assignment, text(assignment, source));
    }
}

// SC2264: `checkBlatantRecursion`.
fn check_recursion(function: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(name) = function
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
    else {
        return;
    };
    let Some(body) = function.child_by_field_name(BODY_FIELD) else {
        return;
    };
    let has_sequence =
        body.kind() == SUBSHELL || (body.kind() == COMPOUND && !is_arithmetic_command(body, source));
    if !has_sequence {
        return;
    }
    if let Some(first) = named_children(body).first() {
        check_leading_statement(*first, name, source, findings);
    }
}

fn check_leading_statement(statement: Node<'_>, name: &str, source: &[u8], findings: &mut Findings) {
    match statement.kind() {
        LIST => {
            if let Some(first) = named_children(statement).first() {
                check_leading_statement(*first, name, source, findings);
            }
        }
        PIPELINE => {
            for stage in named_children(statement) {
                report_if_calls(stage, name, source, findings);
            }
        }
        _ => report_if_calls(statement, name, source, findings),
    }
}

fn report_if_calls(stage: Node<'_>, name: &str, source: &[u8], findings: &mut Findings) {
    let body = if stage.kind() == REDIRECTED {
        stage.child_by_field_name(BODY_FIELD).unwrap_or(stage)
    } else {
        stage
    };
    let Some(word) = Command::of(body).and_then(|command| command.first_word()) else {
        return;
    };
    if literal(word, source).as_deref() == Some(name) {
        findings.report(ShellRule::Sc2264, word, name);
    }
}

// SC2247 y SC2256: `checkDollarQuoteParen` y `checkTranslatedStringVariable`. tree-sitter deja el `$`
// de `$"…"` como un token aparte, pegado al string.
fn check_translated_string(string: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let Some(mark) = string
        .prev_sibling()
        .filter(|mark| !mark.is_named() && text(*mark, source) == TRANSLATION_MARK)
        .filter(|mark| mark.end_byte() == string.start_byte())
    else {
        return;
    };
    let parts = named_children(string);
    let position = Position::of(mark);
    let Some(first) = parts.first().filter(|first| first.kind() == STRING_CONTENT) else {
        return;
    };
    let raw = text(*first, source);
    if raw.starts_with(SUBSTITUTION_STARTS) {
        findings.report_at(ShellRule::Sc2247, position, raw);
    }
    let all_variable_characters = raw
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == VARIABLE_EXTRA);
    if parts.len() == 1 && all_variable_characters && is_variable_name(raw) && is_assigned(raw, context) {
        findings.report_at(ShellRule::Sc2256, position, raw);
    }
}

fn is_assigned(name: &str, context: &Context<'_, '_>) -> bool {
    flow(context)
        .events
        .iter()
        .any(|event| matches!(event.kind, EventKind::Assignment | EventKind::Checked) && event.name == name)
}
