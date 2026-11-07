use tree_sitter::Node;

use super::NumericValues;
use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::parameters::is_variable_name;
use crate::shell::syntax::text;
use crate::shell::test_expression::operator_text;
use crate::shell::variables::{EventKind, flow};
use crate::shell::word_pieces::{
    Piece, PieceKind, flattened, only_literal_text, pieces, simplified_text, simplified_text_is_number,
};

const LESS_OR_GREATER: &[&str] = &["<", "\\<", ">", "\\>"];
const LESS_OR_GREATER_EQUAL: &[&str] = &["<=", "\\<=", ">=", "\\>="];
const REDIRECTING: &[&str] = &["<", ">"];
const ARITHMETIC_OPERATORS: &[&str] = &["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];
const NUMBER_CHARACTERS: &[char] = &['+', '-', '.', ' '];
const SIGNS: &[char] = &['-', '+'];
const DECIMAL_POINT: char = '.';
const NOT_LITERAL: &str = "\0";

#[derive(Clone, Copy)]
pub struct Comparison<'tree> {
    pub operator: Node<'tree>,
    pub left: Node<'tree>,
    pub right: Node<'tree>,
    pub single_bracket: bool,
}

// `checkNumberComparisons` de ShellCheck: SC2071, SC2072, SC2073, SC2122, SC2170 y SC2309.
pub fn check(
    comparison: Comparison<'_>,
    context: &Context<'_, '_>,
    numeric_values: &NumericValues,
    findings: &mut Findings,
) {
    let source = context.source;
    let operator = operator_text(comparison.operator, source);
    let left = pieces(comparison.left, source);
    let right = pieces(comparison.right, source);
    let has_string_comparison = context.dialect != Dialect::Sh;
    let ordering = LESS_OR_GREATER.contains(&operator) || LESS_OR_GREATER_EQUAL.contains(&operator);
    let inclusive_ordering = LESS_OR_GREATER_EQUAL.contains(&operator) && has_string_comparison;
    let is_numeric = |word: Node<'_>, word_pieces: &[Piece]| {
        is_number(word_pieces) || numeric_values.contains_expansion_of(word)
    };
    if is_numeric(comparison.left, &left) || is_numeric(comparison.right, &right) {
        if LESS_OR_GREATER.contains(&operator) || inclusive_ordering {
            findings.report(ShellRule::Sc2071, comparison.operator, operator);
        }
    } else {
        if ordering {
            report_decimals((comparison.left, &left), context, findings);
            report_decimals((comparison.right, &right), context, findings);
        }
        if inclusive_ordering {
            findings.report(ShellRule::Sc2122, comparison.operator, operator);
        }
        if comparison.single_bracket && REDIRECTING.contains(&operator) && context.dialect != Dialect::Sh {
            findings.report(ShellRule::Sc2073, comparison.operator, operator);
        }
    }
    if ARITHMETIC_OPERATORS.contains(&operator) {
        report_decimals((comparison.left, &left), context, findings);
        report_decimals((comparison.right, &right), context, findings);
        let single_bracket = comparison.single_bracket;
        check_string_operand(single_bracket, (comparison.left, &left), context, findings);
        check_string_operand(single_bracket, (comparison.right, &right), context, findings);
    }
}

fn report_decimals(
    (word, word_pieces): (Node<'_>, &[Piece]),
    context: &Context<'_, '_>,
    findings: &mut Findings,
) {
    if context.dialect != Dialect::Ksh && is_fraction(&simplified_text(word_pieces)) {
        findings.report(ShellRule::Sc2072, word, text(word, context.source));
    }
}

fn check_string_operand(
    single_bracket: bool,
    (word, word_pieces): (Node<'_>, &[Piece]),
    context: &Context<'_, '_>,
    findings: &mut Findings,
) {
    if is_numeric_literal(&only_literal_text(word_pieces, "")) {
        return;
    }
    let subject = text(word, context.source);
    if single_bracket {
        findings.report(ShellRule::Sc2170, word, subject);
        return;
    }
    let as_text = only_literal_text(word_pieces, NOT_LITERAL);
    let has_quotes = flattened(word_pieces)
        .iter()
        .any(|piece| matches!(piece.kind, PieceKind::SingleQuoted | PieceKind::DoubleQuoted));
    if !is_variable_name(&as_text) || has_quotes || !is_assigned(&as_text, context) {
        findings.report(ShellRule::Sc2309, word, subject);
    }
}

fn is_assigned(name: &str, context: &Context<'_, '_>) -> bool {
    flow(context)
        .events
        .iter()
        .any(|event| matches!(event.kind, EventKind::Assignment | EventKind::Checked) && event.name == name)
}

fn is_numeric_literal(literal: &str) -> bool {
    literal
        .chars()
        .all(|character| character.is_ascii_digit() || NUMBER_CHARACTERS.contains(&character))
}

fn is_number(word_pieces: &[Piece]) -> bool {
    if matches!(flattened(word_pieces).as_slice(), [only] if only.kind == PieceKind::Arithmetic) {
        return true;
    }
    simplified_text_is_number(&simplified_text(word_pieces))
}

// La expresión `^[-+]?[0-9]+\.[0-9]+$` de ShellCheck.
fn is_fraction(word: &str) -> bool {
    let unsigned = word.strip_prefix(SIGNS).unwrap_or(word);
    unsigned
        .split_once(DECIMAL_POINT)
        .is_some_and(|(whole, fraction)| {
            !whole.is_empty() && !fraction.is_empty() && is_digits(whole) && is_digits(fraction)
        })
}

fn is_digits(text: &str) -> bool {
    text.chars().all(|character| character.is_ascii_digit())
}
