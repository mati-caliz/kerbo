use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Findings;
use crate::shell::syntax::{BODY_FIELD, COMMAND, LIST, NEGATED, REDIRECTED, SUBSHELL, named_children, text};
use crate::shell::word_pieces::{Piece, PieceKind, literal_string, pieces};
use crate::shell::words::oversimplify;

const SINGLE_CLOSE: &str = "]";
const DOUBLE_CLOSE: &str = "]]";
const SINGLE_OPEN: &str = "[";
const DOUBLE_OPEN: &str = "[[";
const EQUALS_ART: &str = "====";
const APPEND: &str = "+=";
const DOUBLE_EQUALS: &str = "==";
const EQUALS: &str = "=";
const ARITHMETIC_OPERATORS: &[&str] = &["+", "-", "*", "/"];
const UNARY_TEST_OPERATORS: &[&str] = &[
    "!", "-a", "-b", "-c", "-d", "-e", "-f", "-g", "-h", "-L", "-k", "-p", "-r", "-s", "-S", "-t", "-u",
    "-w", "-x", "-O", "-G", "-N", "-z", "-n", "-o", "-v", "-R",
];
const BINARY_TEST_OPERATORS: &[&str] = &[
    "-nt", "-ot", "-ef", "==", "!=", "<=", ">=", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "=~", ">", "<",
    "=", "\\<", "\\>", "\\<=", "\\>=",
];

pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() == SUBSHELL {
        check_subshell_as_test(node, source, findings);
    } else if node.kind() == COMMAND
        && let Some(command) = Command::of(node)
    {
        check_second_argument(&command, source, findings);
        check_trailing_bracket(&command, source, findings);
        check_arithmetic_after_assignment(&command, source, findings);
    }
}

// SC2283, SC2284 y SC2285: `checkSecondArgIsComparison`.
fn check_second_argument(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    let [_, argument, ..] = command.words[..] else {
        return;
    };
    let Some(leading) = leading_unquoted(&pieces(argument, source)) else {
        return;
    };
    let subject = text(argument, source);
    if leading.starts_with(EQUALS_ART) {
        return;
    }
    if leading.starts_with(APPEND) {
        findings.report(ShellRule::Sc2285, command.node, subject);
    } else if leading.starts_with(DOUBLE_EQUALS) {
        findings.report(ShellRule::Sc2284, command.node, subject);
    } else if leading.starts_with(EQUALS) {
        findings.report(ShellRule::Sc2283, argument, subject);
    }
}

// `getLeadingUnquotedString`.
fn leading_unquoted(word_pieces: &[Piece]) -> Option<String> {
    let leading: String = word_pieces
        .iter()
        .take_while(|piece| piece.kind == PieceKind::Literal)
        .map(|piece| piece.text.as_str())
        .collect();
    word_pieces
        .first()
        .is_some_and(|first| first.kind == PieceKind::Literal)
        .then_some(leading)
}

// SC2171: `checkTrailingBracket`.
fn check_trailing_bracket(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    let Some(last) = command.words.last().copied() else {
        return;
    };
    if command.is_misread_group(source) {
        return;
    }
    let [only] = &pieces(last, source)[..] else {
        return;
    };
    if only.kind != PieceKind::Literal {
        return;
    }
    let opening = match only.text.as_str() {
        SINGLE_CLOSE => SINGLE_OPEN,
        DOUBLE_CLOSE => DOUBLE_OPEN,
        _ => return,
    };
    let opened = command
        .words
        .iter()
        .any(|word| oversimplify(*word, source) == opening);
    if !opened {
        findings.report(ShellRule::Sc2171, last, text(last, source));
    }
}

// SC2099: `checkArithmeticOpCommand`.
fn check_arithmetic_after_assignment(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    let [_only_assignment] = command.assignments[..] else {
        return;
    };
    let Some(first) = command.first_word() else {
        return;
    };
    let word_text: Option<String> = pieces(first, source)
        .iter()
        .map(|piece| {
            if piece.kind == PieceKind::Glob {
                Some(piece.text.clone())
            } else {
                piece.literal_string()
            }
        })
        .collect();
    if word_text.is_some_and(|word| ARITHMETIC_OPERATORS.contains(&word.as_str())) {
        findings.report(ShellRule::Sc2099, first, text(first, source));
    }
}

// SC2204 y SC2205: `checkSubshellAsTest`.
fn check_subshell_as_test(subshell: Node<'_>, source: &[u8], findings: &mut Findings) {
    let [only] = named_children(subshell)[..] else {
        return;
    };
    let Some(command) = leading_command(only) else {
        return;
    };
    if !command.assignments.is_empty() {
        return;
    }
    let [first, second, ..] = command.words[..] else {
        return;
    };
    let is_in = |word: Node<'_>, operators: &[&str]| {
        literal_string(&pieces(word, source)).is_some_and(|literal| operators.contains(&literal.as_str()))
    };
    let subject = text(subshell, source);
    if is_in(first, UNARY_TEST_OPERATORS) {
        findings.report(ShellRule::Sc2204, subshell, subject);
    }
    if is_in(second, BINARY_TEST_OPERATORS) {
        findings.report(ShellRule::Sc2205, subshell, subject);
    }
}

fn leading_command(node: Node<'_>) -> Option<Command<'_>> {
    match node.kind() {
        NEGATED | LIST => leading_command(*named_children(node).first()?),
        REDIRECTED => leading_command(node.child_by_field_name(BODY_FIELD)?),
        COMMAND => Command::of(node),
        _ => None,
    }
}
