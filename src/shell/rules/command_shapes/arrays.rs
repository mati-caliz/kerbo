use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::syntax::{ARRAY, ASSIGNMENT, NAME_FIELD, SUBSCRIPT, VALUE_FIELD, named_children, text};
use crate::shell::word_pieces::{PieceKind, pieces};

const INDEX_OPEN: char = '[';
const INDEX_VALUE: &str = "]=";
const EQUALS: char = '=';

// SC2190, SC2191 y SC2192: `checkArrayAssignmentIndices`.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != ASSIGNMENT {
        return;
    }
    let source = context.source;
    let Some(target) = node
        .child_by_field_name(NAME_FIELD)
        .filter(|target| target.kind() != SUBSCRIPT)
    else {
        return;
    };
    let Some(array) = node
        .child_by_field_name(VALUE_FIELD)
        .filter(|value| value.kind() == ARRAY)
    else {
        return;
    };
    let is_associative = context.associative.contains(text(target, source));
    for element in named_children(array) {
        check_element(element, is_associative, source, findings);
    }
}

fn check_element(element: Node<'_>, is_associative: bool, source: &[u8], findings: &mut Findings) {
    let raw = text(element, source);
    if raw.starts_with(INDEX_OPEN) && raw.contains(INDEX_VALUE) {
        if raw.ends_with(INDEX_VALUE) {
            let value_start = raw
                .find(INDEX_VALUE)
                .unwrap_or_default()
                .saturating_add(INDEX_VALUE.len());
            findings.report_at(ShellRule::Sc2192, Position::of(element).shifted(value_start), raw);
        }
        return;
    }
    let literal_equals: Vec<Position> = pieces(element, source)
        .into_iter()
        .filter(|piece| {
            piece.kind == PieceKind::Literal
                && piece
                    .text
                    .split_once(EQUALS)
                    .is_some_and(|(before, _)| before.bytes().all(|byte| byte.is_ascii_digit()))
        })
        .map(|piece| piece.position)
        .collect();
    if literal_equals.is_empty() {
        if is_associative {
            findings.report(ShellRule::Sc2190, element, raw);
        }
        return;
    }
    for position in literal_equals {
        findings.report_at(ShellRule::Sc2191, position, raw);
    }
}
