use tree_sitter::Node;

use super::context::Position;
use super::parameters::is_array_expansion;
use super::pattern_scan::{Segment, SegmentKind, segments};
use super::syntax::{
    ANSI_STRING, ARITHMETIC_EXPANSION, COMMAND_SUBSTITUTION, CONCATENATION, EXPANSIONS, NUMBER, RAW_STRING,
    STRING, STRING_CONTENT, TEST_OPERATOR, WORD, children, text,
};
use super::word_scan::scan_unquoted;
use super::words::{braced_content, literal};

const EXTGLOB_PATTERN: &str = "extglob_pattern";
const REGEX: &str = "regex";
const BRACE_EXPRESSION: &str = "brace_expression";
const SINGLE_QUOTE: char = '\'';
const DOUBLE_QUOTE: char = '"';
const TILDE: char = '~';
const CLOSE_BRACKET: char = ']';
const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';
const REGEX_MATCH: &str = "=~";
const HALF_OPEN_RANGE: &str = "[";
const EXPANSION_OPEN: char = '$';
const EXPANDED_PLACEHOLDER: &str = "${VAR}";
const EXPANSION_MARKS: &[char] = &['$', '`'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Literal,
    SingleQuoted,
    DollarSingleQuoted,
    DoubleQuoted,
    Glob,
    Extglob,
    Brace,
    Braced,
    CommandSubstitution,
    Arithmetic,
    Other,
}

// Una parte de un `T_NormalWord` de ShellCheck: tree-sitter deja una palabra con globs como una sola
// hoja, y los chequeos de ShellCheck miran y reportan cada `T_Glob` y `T_Literal` por separado.
#[derive(Debug, Clone)]
pub struct Piece {
    pub kind: PieceKind,
    pub text: String,
    pub position: Position,
    pub inner: Vec<Self>,
}

impl Piece {
    pub(super) const fn new(kind: PieceKind, text: String, position: Position) -> Self {
        Self {
            kind,
            text,
            position,
            inner: Vec::new(),
        }
    }

    // `getLiteralString` de una parte.
    pub fn literal_string(&self) -> Option<String> {
        match self.kind {
            PieceKind::Literal | PieceKind::SingleQuoted | PieceKind::DollarSingleQuoted => {
                Some(self.text.clone())
            }
            PieceKind::DoubleQuoted => self.inner.iter().map(Self::literal_string).collect(),
            PieceKind::Glob
            | PieceKind::Extglob
            | PieceKind::Brace
            | PieceKind::Braced
            | PieceKind::CommandSubstitution
            | PieceKind::Arithmetic
            | PieceKind::Other => None,
        }
    }

    fn is_constant(&self) -> bool {
        match self.kind {
            PieceKind::Literal | PieceKind::SingleQuoted => true,
            PieceKind::DoubleQuoted => self.inner.iter().all(Self::is_constant),
            PieceKind::DollarSingleQuoted
            | PieceKind::Glob
            | PieceKind::Extglob
            | PieceKind::Brace
            | PieceKind::Braced
            | PieceKind::CommandSubstitution
            | PieceKind::Arithmetic
            | PieceKind::Other => false,
        }
    }
}

pub fn pieces(word: Node<'_>, source: &[u8]) -> Vec<Piece> {
    let position = Position::of(word);
    match word.kind() {
        CONCATENATION => concatenation_pieces(word, source),
        EXTGLOB_PATTERN if text(word, source).contains(EXPANSION_MARKS) => {
            mixed_pieces(text(word, source), position)
        }
        WORD | NUMBER | EXTGLOB_PATTERN => scan_unquoted(text(word, source), position),
        TEST_OPERATOR | STRING_CONTENT => vec![Piece::new(
            PieceKind::Literal,
            literal(word, source).unwrap_or_else(|| text(word, source).to_owned()),
            position,
        )],
        REGEX if follows_regex_match(word, source) => vec![regex_piece(text(word, source), position)],
        REGEX => mixed_pieces(text(word, source), position),
        STRING => vec![double_quoted(word, source)],
        RAW_STRING => vec![Piece::new(
            PieceKind::SingleQuoted,
            text(word, source).trim_matches(SINGLE_QUOTE).to_owned(),
            position,
        )],
        ANSI_STRING => vec![Piece::new(
            PieceKind::DollarSingleQuoted,
            literal(word, source).unwrap_or_default(),
            position,
        )],
        BRACE_EXPRESSION => vec![Piece::new(PieceKind::Brace, String::new(), position)],
        COMMAND_SUBSTITUTION => vec![Piece::new(
            PieceKind::CommandSubstitution,
            String::new(),
            position,
        )],
        ARITHMETIC_EXPANSION => vec![Piece::new(PieceKind::Arithmetic, String::new(), position)],
        kind if EXPANSIONS.contains(&kind) => {
            vec![Piece::new(
                PieceKind::Braced,
                braced_content(word, source),
                position,
            )]
        }
        _ if !word.is_named() => vec![Piece::new(
            PieceKind::Literal,
            text(word, source).to_owned(),
            position,
        )],
        _ => vec![Piece::new(PieceKind::Other, String::new(), position)],
    }
}

// Las hojas `word` seguidas se leen juntas: tree-sitter parte `f{a,b}` en cuatro.
fn concatenation_pieces(word: Node<'_>, source: &[u8]) -> Vec<Piece> {
    let mut found = Vec::new();
    let mut run: Option<(usize, usize, Position)> = None;
    for part in children(word) {
        if part.kind() == WORD {
            let start = run.map_or_else(
                || (part.start_byte(), Position::of(part)),
                |(first, _, at)| (first, at),
            );
            run = Some((start.0, part.end_byte(), start.1));
            continue;
        }
        flush_words(&mut run, source, &mut found);
        found.extend(pieces(part, source));
    }
    flush_words(&mut run, source, &mut found);
    found
}

fn flush_words(run: &mut Option<(usize, usize, Position)>, source: &[u8], found: &mut Vec<Piece>) {
    if let Some((start, end, position)) = run.take() {
        let raw = source
            .get(start..end)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or_default();
        found.extend(scan_unquoted(raw, position));
    }
}

fn follows_regex_match(word: Node<'_>, source: &[u8]) -> bool {
    word.prev_sibling()
        .is_some_and(|operator| text(operator, source) == REGEX_MATCH)
}

// El lado derecho de `=~`: lo que no es una expansión es texto, no un glob.
fn regex_piece(raw: &str, position: Position) -> Piece {
    match segments(raw).as_slice() {
        [only] if only.kind == SegmentKind::SingleQuoted => segment_piece(raw, *only, position),
        [only] if only.kind == SegmentKind::DoubleQuoted => segment_piece(raw, *only, position),
        parts if parts.iter().any(|part| is_expansion(part.kind)) => {
            Piece::new(PieceKind::Other, String::new(), position)
        }
        _ => Piece::new(PieceKind::Literal, raw.to_owned(), position),
    }
}

const fn is_expansion(kind: SegmentKind) -> bool {
    matches!(kind, SegmentKind::Expansion | SegmentKind::Substitution)
}

// El lado derecho de `=` en `[[ ]]`, que tree-sitter deja como una sola hoja.
fn mixed_pieces(raw: &str, position: Position) -> Vec<Piece> {
    segments(raw)
        .into_iter()
        .flat_map(|segment| {
            let at = position.shifted(segment.start);
            if segment.kind == SegmentKind::Plain {
                scan_unquoted(raw.get(segment.start..segment.end).unwrap_or_default(), at)
            } else {
                vec![segment_piece(raw, segment, position)]
            }
        })
        .collect()
}

fn segment_piece(raw: &str, segment: Segment, position: Position) -> Piece {
    let contents = raw.get(segment.start..segment.end).unwrap_or_default();
    let at = position.shifted(segment.start);
    match segment.kind {
        SegmentKind::SingleQuoted => Piece::new(
            PieceKind::SingleQuoted,
            contents.trim_matches(SINGLE_QUOTE).to_owned(),
            at,
        ),
        SegmentKind::DoubleQuoted => {
            let inside = contents.trim_matches(DOUBLE_QUOTE);
            let quoted_at = at.shifted(DOUBLE_QUOTE.len_utf8());
            Piece {
                kind: PieceKind::DoubleQuoted,
                text: String::new(),
                position: at,
                inner: double_quoted_contents(inside, quoted_at),
            }
        }
        SegmentKind::Expansion => Piece::new(PieceKind::Braced, expansion_name(contents), at),
        SegmentKind::Substitution => Piece::new(PieceKind::CommandSubstitution, String::new(), at),
        SegmentKind::Plain => Piece::new(PieceKind::Literal, contents.to_owned(), at),
    }
}

fn double_quoted_contents(inside: &str, position: Position) -> Vec<Piece> {
    segments(inside)
        .into_iter()
        .map(|segment| match segment.kind {
            SegmentKind::Expansion | SegmentKind::Substitution => segment_piece(inside, segment, position),
            SegmentKind::Plain | SegmentKind::SingleQuoted | SegmentKind::DoubleQuoted => Piece::new(
                PieceKind::Literal,
                inside
                    .get(segment.start..segment.end)
                    .unwrap_or_default()
                    .to_owned(),
                position.shifted(segment.start),
            ),
        })
        .collect()
}

fn expansion_name(contents: &str) -> String {
    let inner = contents.strip_prefix(EXPANSION_OPEN).unwrap_or(contents);
    inner
        .strip_prefix(OPEN_BRACE)
        .and_then(|braced| braced.strip_suffix(CLOSE_BRACE))
        .unwrap_or(inner)
        .to_owned()
}

// El espacio entre una expansión y las comillas que cierran queda adentro del token de cierre.
fn double_quoted(string: Node<'_>, source: &[u8]) -> Piece {
    let mut inner = Vec::new();
    for child in children(string) {
        if child.is_named() {
            inner.extend(pieces(child, source));
        } else if let Some(trailing) = text(child, source)
            .strip_suffix(DOUBLE_QUOTE)
            .filter(|trailing| !trailing.is_empty())
        {
            inner.push(Piece::new(
                PieceKind::Literal,
                trailing.to_owned(),
                Position::of(child),
            ));
        }
    }
    Piece {
        kind: PieceKind::DoubleQuoted,
        text: String::new(),
        position: Position::of(string),
        inner,
    }
}

// `getWordParts`: lo de adentro de las comillas dobles queda suelto.
pub fn flattened(word_pieces: &[Piece]) -> Vec<&Piece> {
    word_pieces
        .iter()
        .flat_map(|piece| {
            if piece.kind == PieceKind::DoubleQuoted {
                piece.inner.iter().collect()
            } else {
                vec![piece]
            }
        })
        .collect()
}

// `isConstant`, que no considera constante una palabra que empieza con `~`.
pub fn is_constant(word_pieces: &[Piece]) -> bool {
    let starts_with_tilde = word_pieces
        .first()
        .is_some_and(|first| first.kind == PieceKind::Literal && first.text.starts_with(TILDE));
    !starts_with_tilde && word_pieces.iter().all(Piece::is_constant)
}

// `isGlob`: un glob sin comillas, o `foo[x${var}y]`, que ShellCheck parte en `[`, `x`, `$var` e `y]`.
pub fn has_glob(word_pieces: &[Piece]) -> bool {
    word_pieces
        .iter()
        .any(|piece| matches!(piece.kind, PieceKind::Glob | PieceKind::Extglob))
        || word_pieces
            .iter()
            .skip_while(|piece| !is_half_open_range(piece))
            .any(|piece| piece.kind == PieceKind::Literal && piece.text.contains(CLOSE_BRACKET))
}

fn is_half_open_range(piece: &Piece) -> bool {
    piece.kind == PieceKind::Literal && piece.text == HALF_OPEN_RANGE
}

pub fn has_array_expansion(word_pieces: &[Piece]) -> bool {
    flattened(word_pieces)
        .into_iter()
        .any(|piece| piece.kind == PieceKind::Braced && is_array_expansion(&piece.text))
}

pub fn has_brace_expansion(word_pieces: &[Piece]) -> bool {
    flattened(word_pieces)
        .into_iter()
        .any(|piece| piece.kind == PieceKind::Brace)
}

// `getLiteralString` de la palabra entera.
pub fn literal_string(word_pieces: &[Piece]) -> Option<String> {
    word_pieces.iter().map(Piece::literal_string).collect()
}

// `getLiteralStringDef`: lo que no es texto fijo vale lo que se indique.
pub fn only_literal_text(word_pieces: &[Piece], not_literal: &str) -> String {
    flattened(word_pieces)
        .into_iter()
        .map(|piece| piece.literal_string().unwrap_or_else(|| not_literal.to_owned()))
        .collect()
}

// `oversimplify` de una palabra en partes: los globs quedan como están y las expansiones son `${VAR}`.
pub fn simplified_text(word_pieces: &[Piece]) -> String {
    word_pieces
        .iter()
        .map(|piece| match piece.kind {
            PieceKind::DoubleQuoted => simplified_text(&piece.inner),
            PieceKind::Braced | PieceKind::CommandSubstitution | PieceKind::Arithmetic => {
                EXPANDED_PLACEHOLDER.to_owned()
            }
            PieceKind::Literal
            | PieceKind::SingleQuoted
            | PieceKind::DollarSingleQuoted
            | PieceKind::Glob
            | PieceKind::Extglob
            | PieceKind::Brace
            | PieceKind::Other => piece.text.clone(),
        })
        .collect()
}

pub fn simplified_text_is_number(word: &str) -> bool {
    word.chars().all(|character| character.is_ascii_digit())
}

// `getUnquotedLiteral`: la palabra sólo tiene texto sin comillas.
pub fn unquoted_literal(word_pieces: &[Piece]) -> Option<String> {
    word_pieces
        .iter()
        .map(|piece| (piece.kind == PieceKind::Literal).then(|| piece.text.clone()))
        .collect()
}
