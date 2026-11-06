use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::pseudo_glob::{PseudoGlob, can_overlap, exact, fuzzy, is_superset};
use crate::shell::syntax::{CASE, CASE_ITEM, VALUE_FIELD, field_children, named_children, text};
use crate::shell::word_pieces::{Piece, PieceKind, has_glob, is_constant, pieces};

const FALLTHROUGH_FIELD: &str = "fallthrough";

// `checkUnmatchableCases` de ShellCheck: SC2194, SC2195, SC2221 y SC2222.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != CASE {
        return;
    }
    let source = context.source;
    let Some(subject) = node.child_by_field_name(VALUE_FIELD) else {
        return;
    };
    let items: Vec<Node<'_>> = named_children(node)
        .into_iter()
        .filter(|child| child.kind() == CASE_ITEM)
        .collect();
    let subject_pieces = pieces(subject, source);
    if is_constant(&subject_pieces) {
        findings.report(ShellRule::Sc2194, subject, text(subject, source));
    } else {
        let subject_glob = fuzzy(&subject_pieces);
        for pattern in items.iter().flat_map(|item| patterns(*item, source)) {
            if !can_overlap(&subject_glob, &fuzzy(&pattern.pieces)) {
                findings.report(ShellRule::Sc2195, pattern.first, text(pattern.first, source));
            }
        }
    }
    check_shadowed_patterns(&items, source, findings);
    for pattern in items.iter().flat_map(|item| patterns(*item, source)) {
        let has_expansion = pattern
            .pieces
            .iter()
            .any(|piece| matches!(piece.kind, PieceKind::Braced | PieceKind::CommandSubstitution));
        if has_expansion && !has_glob(&pattern.pieces) {
            findings.report(ShellRule::Sc2254, pattern.first, text(pattern.first, source));
        }
    }
}

struct Pattern<'tree> {
    first: Node<'tree>,
    pieces: Vec<Piece>,
}

// tree-sitter deja `*"a"*` como varios `value` seguidos, uno por parte: sólo un `|` separa patrones.
fn patterns<'tree>(item: Node<'tree>, source: &[u8]) -> Vec<Pattern<'tree>> {
    let mut found: Vec<Pattern<'tree>> = Vec::new();
    let mut previous_end = None;
    for value in field_children(item, VALUE_FIELD) {
        let word_pieces = pieces(value, source);
        match found
            .last_mut()
            .filter(|_| previous_end == Some(value.start_byte()))
        {
            Some(pattern) => pattern.pieces.extend(word_pieces),
            None => found.push(Pattern {
                first: value,
                pieces: word_pieces,
            }),
        }
        previous_end = Some(value.end_byte());
    }
    found
}

fn check_shadowed_patterns(items: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let breaking: Vec<Pattern<'_>> = items
        .iter()
        .filter(|item| item.child_by_field_name(FALLTHROUGH_FIELD).is_none())
        .flat_map(|item| patterns(*item, source))
        .collect();
    let fuzzy_globs: Vec<Vec<PseudoGlob>> = breaking.iter().map(|pattern| fuzzy(&pattern.pieces)).collect();
    for (index, pattern) in breaking.iter().enumerate() {
        let Some(general) = exact(&pattern.pieces) else {
            continue;
        };
        let later = breaking
            .iter()
            .zip(fuzzy_globs.iter())
            .skip(index.saturating_add(1))
            .find(|(_, glob)| is_superset(&general, glob));
        if let Some((shadowed, _)) = later {
            findings.report(ShellRule::Sc2221, pattern.first, text(pattern.first, source));
            findings.report(ShellRule::Sc2222, shadowed.first, text(shadowed.first, source));
        }
    }
}
