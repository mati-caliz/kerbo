use super::parameters::is_array_expansion;
use super::word_pieces::{Piece, PieceKind};

const INDIRECT: char = '!';

// `willSplit`.
pub fn will_split(piece: &Piece) -> bool {
    match piece.kind {
        PieceKind::Braced
        | PieceKind::CommandSubstitution
        | PieceKind::Brace
        | PieceKind::Glob
        | PieceKind::Extglob => true,
        PieceKind::DoubleQuoted => piece.inner.iter().any(will_become_multiple),
        PieceKind::Literal
        | PieceKind::SingleQuoted
        | PieceKind::DollarSingleQuoted
        | PieceKind::Arithmetic
        | PieceKind::Other => false,
    }
}

// `willBecomeMultipleArgs`.
fn will_become_multiple(piece: &Piece) -> bool {
    matches!(
        piece.kind,
        PieceKind::Extglob | PieceKind::Glob | PieceKind::Brace
    ) || concatenates_in_assignment(piece)
}

const fn is_not_expansion(kind: PieceKind) -> bool {
    matches!(
        kind,
        PieceKind::Literal
            | PieceKind::SingleQuoted
            | PieceKind::DollarSingleQuoted
            | PieceKind::Glob
            | PieceKind::Extglob
            | PieceKind::Brace
            | PieceKind::CommandSubstitution
            | PieceKind::Arithmetic
            | PieceKind::Other
    )
}

// `willConcatInAssignment`.
fn concatenates_in_assignment(piece: &Piece) -> bool {
    if is_not_expansion(piece.kind) {
        return false;
    }
    if piece.kind == PieceKind::Braced {
        return is_array_expansion(&piece.text);
    }
    piece.inner.iter().any(concatenates_in_assignment)
}

// `mayBecomeMultipleArgs`.
pub fn may_become_multiple(word_pieces: &[Piece]) -> bool {
    word_pieces
        .iter()
        .any(|piece| will_become_multiple(piece) || expands_to_several(piece, false))
}

fn expands_to_several(piece: &Piece, quoted: bool) -> bool {
    if is_not_expansion(piece.kind) {
        return false;
    }
    if piece.kind == PieceKind::Braced {
        return !quoted || piece.text.starts_with(INDIRECT);
    }
    piece.inner.iter().any(|inner| expands_to_several(inner, true))
}
