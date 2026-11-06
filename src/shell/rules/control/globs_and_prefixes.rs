use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Findings;
use crate::shell::parameters::reference;
use crate::shell::syntax::{
    ARRAY, BINARY, COMMAND, CONCATENATION, EXPANSIONS, NAME_FIELD, OPERATOR_FIELD, TEST, UNARY, WORD,
    ancestors, is_double_bracket, text,
};
use crate::shell::variables::assigned_name;
use crate::shell::word_pieces::{Piece, PieceKind, pieces};
use crate::shell::words::braced_content;

const REGEX: &str = "regex";
const EXTGLOB_PATTERN: &str = "extglob_pattern";
const REGEX_MATCH: &str = "=~";
const OPEN_BRACKET: char = '[';
const INDEX_VALUE: &str = "]=";
const ESCAPE: char = '\\';
const CLOSE_BRACKET: char = ']';
const COLON: char = ':';
const ONLY_COLON: &str = ":";
const NEGATIONS: &[char] = &['!', '^'];
const RANGE: char = '-';
const IGNORED_COMMANDS: &[&str] = &["tr", "read"];
const VARIABLE_IS_SET: &str = "-v";
const ARITHMETIC_OPERATORS: &[&str] = &["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];

pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if EXPANSIONS.contains(&node.kind()) {
        check_prefix_reference(node, source, findings);
    } else if is_whole_word(node) && !is_regex_operand(node, source) {
        check_character_ranges(node, source, findings);
    }
}

fn is_whole_word(node: Node<'_>) -> bool {
    if node.kind() == CONCATENATION {
        return true;
    }
    [WORD, EXTGLOB_PATTERN, REGEX].contains(&node.kind())
        && node.parent().is_none_or(|parent| parent.kind() != CONCATENATION)
}

fn is_regex_operand(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == REGEX
        && node
            .prev_sibling()
            .is_some_and(|operator| text(operator, source) == REGEX_MATCH)
}

// SC2097 y SC2098: `checkPrefixAssignmentReference`.
fn check_prefix_reference(expansion: Node<'_>, source: &[u8], findings: &mut Findings) {
    let name = reference(&braced_content(expansion, source));
    for ancestor in ancestors(expansion) {
        let Some(command) = Command::of(ancestor).filter(|command| command.node.kind() == COMMAND) else {
            continue;
        };
        if command.words.is_empty() {
            continue;
        }
        for assignment in command.assignments {
            let is_plain = assignment
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|target| target.kind() != "subscript");
            let is_outside = !ancestors(expansion).any(|other| other == assignment);
            if is_plain && is_outside && assigned_name(assignment, source).as_deref() == Some(name.as_str()) {
                findings.report(ShellRule::Sc2097, assignment, &name);
                findings.report(ShellRule::Sc2098, expansion, &name);
            }
        }
        return;
    }
}

// SC2101 y SC2102: `checkCharRangeGlob`.
fn check_character_ranges(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    let classes: Vec<Piece> = pieces(word, source)
        .into_iter()
        .filter(|piece| {
            piece.kind == PieceKind::Glob
                && piece.text.starts_with(OPEN_BRACKET)
                && piece.text.ends_with(CLOSE_BRACKET)
                && !piece.text.contains(ESCAPE)
        })
        .collect();
    if classes.is_empty()
        || is_array_index(word, source)
        || ancestors(word).any(|ancestor| EXPANSIONS.contains(&ancestor.kind()))
        || is_in_ignored_command(word, source)
        || is_dereferenced(word, source)
    {
        return;
    }
    for class in classes {
        let inner = class
            .text
            .strip_prefix(OPEN_BRACKET)
            .and_then(|rest| rest.strip_suffix(CLOSE_BRACKET))
            .unwrap_or_default();
        let contents = inner.strip_prefix(NEGATIONS).unwrap_or(inner);
        if contents.starts_with(COLON) && contents.ends_with(COLON) && contents != ONLY_COLON {
            findings.report_at(ShellRule::Sc2101, class.position, &class.text);
        } else if !contents.contains(OPEN_BRACKET) && has_duplicates(contents) {
            findings.report_at(ShellRule::Sc2102, class.position, &class.text);
        }
    }
}

// `a=([foo]=b)`: el corchete es un índice.
fn is_array_index(word: Node<'_>, source: &[u8]) -> bool {
    word.parent().is_some_and(|parent| parent.kind() == ARRAY)
        && text(word, source).starts_with(OPEN_BRACKET)
        && text(word, source).contains(INDEX_VALUE)
}

fn has_duplicates(contents: &str) -> bool {
    let mut letters: Vec<char> = contents.chars().filter(|letter| *letter != RANGE).collect();
    letters.sort_unstable();
    letters.windows(2).any(|pair| pair.first() == pair.get(1))
}

fn is_in_ignored_command(word: Node<'_>, source: &[u8]) -> bool {
    ancestors(word)
        .find_map(Command::of)
        .and_then(|command| command.name(source))
        .is_some_and(|name| IGNORED_COMMANDS.contains(&name.as_str()))
}

// `[[ -v a[b] ]]` y `[[ a[b] -eq 1 ]]`: el corchete es un índice, no un glob.
fn is_dereferenced(word: Node<'_>, source: &[u8]) -> bool {
    for ancestor in ancestors(word) {
        match ancestor.kind() {
            UNARY => {
                let operator = ancestor
                    .child_by_field_name(OPERATOR_FIELD)
                    .map_or("", |operator| text(operator, source));
                return operator == VARIABLE_IS_SET;
            }
            BINARY => {
                let operator = ancestor
                    .child_by_field_name(OPERATOR_FIELD)
                    .map_or("", |operator| text(operator, source));
                let in_double_bracket = ancestors(ancestor)
                    .find(|outer| outer.kind() == TEST)
                    .is_some_and(|test| is_double_bracket(test, source));
                return in_double_bracket && ARITHMETIC_OPERATORS.contains(&operator);
            }
            COMMAND => return false,
            _ => {}
        }
    }
    false
}
