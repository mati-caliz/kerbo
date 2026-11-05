use super::word_pieces::{Piece, PieceKind, flattened};

const TILDE: char = '~';
const PATH_SEPARATOR: char = '/';
const ANY_CHARACTER: &str = "?";
const ANY_TEXT: &str = "*";
const CLASS_START: char = '[';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoGlob {
    Any,
    Many,
    Character(char),
}

// `wordToPseudoGlob` de ShellCheck: lo que no se puede leer cuenta como cualquier texto.
pub fn fuzzy(word_pieces: &[Piece]) -> Vec<PseudoGlob> {
    simplified(convert(word_pieces, false).unwrap_or_else(|| vec![PseudoGlob::Many]))
}

// `wordToExactPseudoGlob`: sólo si el patrón se puede representar sin perder nada.
pub fn exact(word_pieces: &[Piece]) -> Option<Vec<PseudoGlob>> {
    convert(word_pieces, true).map(simplified)
}

fn convert(word_pieces: &[Piece], exact: bool) -> Option<Vec<PseudoGlob>> {
    let mut remaining = word_pieces;
    let mut converted = Vec::new();
    if let Some((first, rest)) = word_pieces.split_first()
        && first.kind == PieceKind::Literal
        && let Some(after_tilde) = first.text.strip_prefix(TILDE)
    {
        if exact {
            return None;
        }
        converted.push(PseudoGlob::Many);
        converted.extend(
            after_tilde
                .chars()
                .skip_while(|character| *character != PATH_SEPARATOR)
                .map(PseudoGlob::Character),
        );
        remaining = rest;
    }
    for piece in flattened(remaining) {
        converted.extend(piece_glob(piece, exact)?);
    }
    Some(converted)
}

fn piece_glob(piece: &Piece, exact: bool) -> Option<Vec<PseudoGlob>> {
    match piece.kind {
        PieceKind::Literal | PieceKind::SingleQuoted => {
            return Some(piece.text.chars().map(PseudoGlob::Character).collect());
        }
        PieceKind::Glob if piece.text == ANY_CHARACTER => return Some(vec![PseudoGlob::Any]),
        PieceKind::Glob if piece.text == ANY_TEXT => return Some(vec![PseudoGlob::Many]),
        PieceKind::Glob if !exact && piece.text.starts_with(CLASS_START) => {
            return Some(vec![PseudoGlob::Any]);
        }
        PieceKind::Glob
        | PieceKind::DollarSingleQuoted
        | PieceKind::DoubleQuoted
        | PieceKind::Extglob
        | PieceKind::Brace
        | PieceKind::Braced
        | PieceKind::CommandSubstitution
        | PieceKind::Arithmetic
        | PieceKind::Other => {}
    }
    (!exact).then(|| vec![PseudoGlob::Many])
}

// `f?*?**g` queda como `f??*g`: un tramo de comodines pone primero los `?` y un solo `*`.
fn simplified(globs: Vec<PseudoGlob>) -> Vec<PseudoGlob> {
    let mut result = Vec::with_capacity(globs.len());
    let mut any_count = 0_usize;
    let mut has_many = false;
    for glob in globs {
        match glob {
            PseudoGlob::Any => any_count = any_count.saturating_add(1),
            PseudoGlob::Many => has_many = true,
            PseudoGlob::Character(_) => {
                flush_wildcards(&mut result, &mut any_count, &mut has_many);
                result.push(glob);
            }
        }
    }
    flush_wildcards(&mut result, &mut any_count, &mut has_many);
    result
}

fn flush_wildcards(result: &mut Vec<PseudoGlob>, any_count: &mut usize, has_many: &mut bool) {
    result.extend(std::iter::repeat_n(PseudoGlob::Any, *any_count));
    if *has_many {
        result.push(PseudoGlob::Many);
    }
    *any_count = 0;
    *has_many = false;
}

// `pseudoGlobsCanOverlap`.
pub fn can_overlap(left: &[PseudoGlob], right: &[PseudoGlob]) -> bool {
    match (left, right) {
        ([], []) => true,
        ([PseudoGlob::Many, rest @ ..], []) | ([], [PseudoGlob::Many, rest @ ..]) => can_overlap(rest, &[]),
        ([_, ..], []) | ([], [_, ..]) => false,
        ([left_first, left_rest @ ..], [right_first, right_rest @ ..]) => match (left_first, right_first) {
            (PseudoGlob::Many, _) | (_, PseudoGlob::Many) => {
                can_overlap(left, right_rest) || can_overlap(left_rest, right)
            }
            (PseudoGlob::Any, _) | (_, PseudoGlob::Any) => can_overlap(left_rest, right_rest),
            (PseudoGlob::Character(_), PseudoGlob::Character(_)) => {
                left_first == right_first && can_overlap(left_rest, right_rest)
            }
        },
    }
}

// `pseudoGlobIsSuperSetof`.
pub fn is_superset(general: &[PseudoGlob], specific: &[PseudoGlob]) -> bool {
    match (general, specific) {
        ([], []) => true,
        ([PseudoGlob::Many, rest @ ..], []) => is_superset(rest, &[]),
        ([_, ..], []) | ([], [_, ..]) => false,
        ([general_first, general_rest @ ..], [specific_first, specific_rest @ ..]) => {
            match (general_first, specific_first) {
                (PseudoGlob::Many, PseudoGlob::Many) => is_superset(general, specific_rest),
                (PseudoGlob::Many, _) => {
                    is_superset(general, specific_rest) || is_superset(general_rest, specific)
                }
                (_, PseudoGlob::Many) => false,
                (PseudoGlob::Any, _) => is_superset(general_rest, specific_rest),
                (_, PseudoGlob::Any) => false,
                (PseudoGlob::Character(_), PseudoGlob::Character(_)) => {
                    general_first == specific_first && is_superset(general_rest, specific_rest)
                }
            }
        }
    }
}
