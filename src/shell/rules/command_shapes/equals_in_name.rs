use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::dialect::Dialect;
use crate::shell::parameters::{is_variable_name, modifier, reference};
use crate::shell::syntax::{ASSIGNMENT, ASSIGNMENTS, COMMAND, DECLARATION, NAME_FIELD, text};
use crate::shell::word_pieces::{Piece, PieceKind, pieces};
use crate::shell::word_scan::scan_unquoted;

const EQUALS: char = '=';
const FLAG_PREFIX: char = '-';
const APPEND_PLUS: char = '+';
const DOUBLE_EQUALS: &str = "==";
const TRIPLE_EQUALS: &str = "===";
const POSITIONAL_ZERO: &str = "0=";
const ARGUMENT_COUNT_PREFIX: char = '#';
const ZERO: &str = "0";
const OPEN_BRACKET: char = '[';
const CLOSE_BRACKET: char = ']';
const MIN_CONFLICT_MARKER: usize = 4;
const MAX_CONFLICT_MARKER: usize = 12;
const VARIABLE_EXTRA: char = '_';
const ANY_NAME: &str = "x";

struct EqualsLiteral<'word> {
    text: &'word str,
    position: Position,
}

struct NameWord<'word> {
    command_start: Position,
    word_start: Position,
    is_alone: bool,
    pieces: &'word [Piece],
}

// SC2270 a SC2282: `checkEqualsInCommand`. tree-sitter lee `1=2` como una asignación a una variable
// llamada `1`, que para ShellCheck es un comando.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() == COMMAND {
        check_command(node, context, findings);
    } else if node.kind() == ASSIGNMENT {
        check_numeric_assignment(node, context, findings);
    }
}

fn check_command(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let Some(first) = command.first_word() else {
        return;
    };
    let word_pieces = pieces(first, context.source);
    let name = NameWord {
        command_start: Position::of(node),
        word_start: Position::of(first),
        is_alone: command.assignments.is_empty() && command.words.len() == 1,
        pieces: &word_pieces,
    };
    report(&name, context.dialect, findings);
}

fn check_numeric_assignment(assignment: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let is_alone = assignment
        .parent()
        .is_none_or(|parent| ![COMMAND, DECLARATION, ASSIGNMENTS].contains(&parent.kind()));
    let starts_with_digit = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source).starts_with(|first: char| first.is_ascii_digit()));
    if !is_alone || !starts_with_digit {
        return;
    }
    let position = Position::of(assignment);
    let word_pieces = scan_unquoted(text(assignment, source), position);
    let name = NameWord {
        command_start: position,
        word_start: position,
        is_alone: true,
        pieces: &word_pieces,
    };
    report(&name, context.dialect, findings);
}

fn report(name: &NameWord<'_>, dialect: Dialect, findings: &mut Findings) {
    let Some(index) = name
        .pieces
        .iter()
        .position(|piece| piece.kind == PieceKind::Literal && piece.text.contains(EQUALS))
    else {
        return;
    };
    let (leading, rest) = name.pieces.split_at(index);
    let Some(equals) = rest.first() else {
        return;
    };
    let unprefixed = equals
        .text
        .strip_prefix(APPEND_PLUS)
        .filter(|text| text.starts_with(EQUALS));
    let leading = strip_single_plus(leading);
    let literal = unprefixed.unwrap_or(&equals.text);
    let target = EqualsLiteral {
        text: literal,
        position: equals.position,
    };
    if let Some((rule, position)) = classify(name, leading, &target, dialect) {
        findings.report_at(rule, position, literal);
    }
}

fn strip_single_plus(leading: &[Piece]) -> &[Piece] {
    match leading.split_last() {
        Some((last, rest))
            if last.kind == PieceKind::Literal
                && last.text.len() == 1
                && last.text.starts_with(APPEND_PLUS) =>
        {
            rest
        }
        _ => leading,
    }
}

fn classify(
    name: &NameWord<'_>,
    leading: &[Piece],
    target: &EqualsLiteral<'_>,
    dialect: Dialect,
) -> Option<(ShellRule, Position)> {
    let literal = target.text;
    let literal_position = target.position;
    if leading.is_empty() && (literal.starts_with(FLAG_PREFIX) || is_misread_assignment(literal)) {
        return None;
    }
    if leading.is_empty() && literal.starts_with(EQUALS) {
        return Some(classify_border(name, literal));
    }
    if literal.contains(DOUBLE_EQUALS) {
        return Some((ShellRule::Sc2272, name.word_start));
    }
    if let [only] = leading
        && only.kind == PieceKind::Braced
        && literal.starts_with(EQUALS)
    {
        return Some(classify_braced_target(name, only, dialect));
    }
    if leading.is_empty() {
        if is_positional_assignment(literal) {
            return Some(if literal.starts_with(POSITIONAL_ZERO) {
                (assign_zero_rule(dialect), literal_position)
            } else {
                (ShellRule::Sc2270, literal_position)
            });
        }
        if is_leading_number_variable(literal) {
            return Some((ShellRule::Sc2282, name.word_start));
        }
    }
    let name_part = literal.split(EQUALS).next().unwrap_or_default();
    if !leading.is_empty() && may_be_variable_name(leading) && name_part.chars().all(is_variable_character) {
        return Some((ShellRule::Sc2271, name.word_start));
    }
    Some((ShellRule::Sc2276, name.word_start))
}

// tree-sitter lee `_="$x"` como un comando llamado `_=`, y para ShellCheck es una asignación.
fn is_misread_assignment(literal: &str) -> bool {
    literal
        .split_once(EQUALS)
        .is_some_and(|(name, _)| is_variable_name(name))
}

fn classify_border(name: &NameWord<'_>, literal: &str) -> (ShellRule, Position) {
    let is_conflict_marker = name.is_alone
        && matches!(name.pieces, [only] if only.kind == PieceKind::Literal && is_marker_text(&only.text));
    if is_conflict_marker {
        (ShellRule::Sc2273, name.command_start)
    } else if literal.starts_with(TRIPLE_EQUALS) {
        (ShellRule::Sc2274, name.command_start)
    } else {
        (ShellRule::Sc2275, name.word_start)
    }
}

fn is_marker_text(text: &str) -> bool {
    text.chars().all(|character| character == EQUALS)
        && (MIN_CONFLICT_MARKER..=MAX_CONFLICT_MARKER).contains(&text.chars().count())
}

fn classify_braced_target(name: &NameWord<'_>, braced: &Piece, dialect: Dialect) -> (ShellRule, Position) {
    let content = braced.text.as_str();
    if content.is_empty() || content.starts_with(ARGUMENT_COUNT_PREFIX) {
        return (ShellRule::Sc2276, name.word_start);
    }
    if content == ZERO {
        return (assign_zero_rule(dialect), braced.position);
    }
    if content.chars().all(|character| character.is_ascii_digit()) {
        return (ShellRule::Sc2270, braced.position);
    }
    let name_modifier = modifier(content);
    let is_array = !reference(content).is_empty()
        && name_modifier.starts_with(OPEN_BRACKET)
        && name_modifier.ends_with(CLOSE_BRACKET);
    if is_array || is_variable_name(content) {
        (ShellRule::Sc2281, braced.position)
    } else {
        (ShellRule::Sc2271, braced.position)
    }
}

const fn assign_zero_rule(dialect: Dialect) -> ShellRule {
    match dialect {
        Dialect::Bash => ShellRule::Sc2277,
        Dialect::Ksh => ShellRule::Sc2278,
        Dialect::Dash | Dialect::Busybox => ShellRule::Sc2279,
        Dialect::Sh => ShellRule::Sc2280,
    }
}

// La expresión `^[0-9][0-9]?=` de ShellCheck.
fn is_positional_assignment(literal: &str) -> bool {
    let Some((digits, _)) = literal.split_once(EQUALS) else {
        return false;
    };
    (1..=2).contains(&digits.len()) && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_leading_number_variable(literal: &str) -> bool {
    let lead = literal.split(EQUALS).next().unwrap_or_default();
    lead.starts_with(|first: char| first.is_ascii_digit())
        && lead.chars().all(is_variable_character)
        && !lead.bytes().all(|byte| byte.is_ascii_digit())
}

const fn is_variable_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == VARIABLE_EXTRA
}

// `mayBeVariableName`: las expansiones valen como una letra.
fn may_be_variable_name(leading: &[Piece]) -> bool {
    let mut name = String::new();
    for piece in leading {
        match piece.kind {
            PieceKind::Literal => name.push_str(&piece.text),
            PieceKind::Braced
            | PieceKind::CommandSubstitution
            | PieceKind::Arithmetic
            | PieceKind::DollarSingleQuoted
            | PieceKind::Other => {
                name.push_str(ANY_NAME);
            }
            PieceKind::SingleQuoted
            | PieceKind::DoubleQuoted
            | PieceKind::Glob
            | PieceKind::Extglob
            | PieceKind::Brace => {
                return false;
            }
        }
    }
    is_variable_name(&name)
}
