use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{ARRAY, CONCATENATION, STRING, ancestors, text};
use crate::shell::word_pieces::{Piece, PieceKind, pieces};

const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const EVAL: &str = "eval";
const VARIABLE_BEFORE_RANGE: &str = "$..";
const VARIABLE_AFTER_RANGE: &str = "..$";
const EXPANSION_MARK: char = '$';
const OTHER_MARK: char = '-';
const INDEX_OPEN: char = '[';
const INDEX_CLOSE_OPEN: &str = "][";
const INDEX_VALUE: &str = "]=";

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if context.dialect != Dialect::Bash {
        return;
    }
    if node.kind() == CONCATENATION
        && node
            .parent()
            .is_none_or(|parent| ![CONCATENATION, STRING].contains(&parent.kind()))
    {
        check_range_variables(node, context.source, findings);
        check_multidimensional_element(node, context.source, findings);
    }
}

// SC2051 y SC2175: `checkBraceExpansionVars`.
fn check_range_variables(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    let word_pieces = pieces(word, source);
    let Some(opening) = word_pieces
        .iter()
        .find(|piece| piece.kind == PieceKind::Literal && piece.text.contains(OPEN_BRACE))
    else {
        return;
    };
    let shape: String = word_pieces.iter().map(shape_of).collect();
    let Some(content) = shape
        .split_once(OPEN_BRACE)
        .and_then(|(_, after)| after.split_once(CLOSE_BRACE))
        .map(|(content, _)| content)
    else {
        return;
    };
    if !content.contains(VARIABLE_BEFORE_RANGE) && !content.contains(VARIABLE_AFTER_RANGE) {
        return;
    }
    let offset = opening.text.find(OPEN_BRACE).unwrap_or_default();
    let position = opening.position.shifted(offset);
    let is_evaled = ancestors(word)
        .find_map(Command::of)
        .is_some_and(|command| command.literal_name(source).as_deref() == Some(EVAL));
    let rule = if is_evaled {
        ShellRule::Sc2175
    } else {
        ShellRule::Sc2051
    };
    findings.report_at(rule, position, text(word, source));
}

fn shape_of(piece: &Piece) -> String {
    match piece.kind {
        PieceKind::Literal => piece.text.clone(),
        PieceKind::Braced | PieceKind::CommandSubstitution | PieceKind::Arithmetic => {
            EXPANSION_MARK.to_string()
        }
        PieceKind::SingleQuoted
        | PieceKind::DollarSingleQuoted
        | PieceKind::DoubleQuoted
        | PieceKind::Glob
        | PieceKind::Extglob
        | PieceKind::Brace
        | PieceKind::Other => OTHER_MARK.to_string(),
    }
}

// SC2180: `checkMultiDimensionalArrays`, en un elemento `a=([1][2]=3)`.
fn check_multidimensional_element(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    if word.parent().is_none_or(|parent| parent.kind() != ARRAY) {
        return;
    }
    let raw = text(word, source);
    if !raw.starts_with(INDEX_OPEN) {
        return;
    }
    let Some((index, _)) = raw.split_once(INDEX_VALUE) else {
        return;
    };
    if let Some(offset) = index.find(INDEX_CLOSE_OPEN) {
        let position =
            crate::shell::context::Position::of(word).shifted(offset.saturating_add(INDEX_CLOSE_OPEN.len()));
        findings.report_at(ShellRule::Sc2180, position, raw);
    }
}
