use tree_sitter::Point;

use super::Context;
use super::nodes::STRING;
use crate::lint::{LintFinding, LintKind, finding_at};
use crate::rules::EslintRule;

const LINE_TERMINATORS: &[char] = &['\u{2028}', '\u{2029}'];
const IRREGULAR: &[char] = &[
    '\u{c}', '\u{b}', '\u{85}', '\u{feff}', '\u{a0}', '\u{1680}', '\u{180e}', '\u{2000}', '\u{2001}',
    '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200a}', '\u{200b}', '\u{202f}', '\u{205f}', '\u{3000}',
];
const BYTE_ORDER_MARK: char = '\u{feff}';

// `no-irregular-whitespace`, con sus opciones por defecto: lo que está adentro de un string no cuenta, y
// las líneas son las de ESLint, que también corta en U+2028 y U+2029.
pub fn irregular_whitespace(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let Ok(source) = std::str::from_utf8(context.source) else {
        return Vec::new();
    };
    if !source.contains(|character| IRREGULAR.contains(&character) || LINE_TERMINATORS.contains(&character)) {
        return Vec::new();
    }
    let strings: Vec<(usize, usize)> = context
        .of_kind(&[STRING])
        .map(|string| (string.start_byte(), string.end_byte()))
        .collect();
    let is_in_string = |byte: usize| strings.iter().any(|(start, end)| *start <= byte && byte < *end);
    let mut found = Vec::new();
    let mut row = 0;
    let mut line_start = 0;
    let mut in_run = false;
    let mut characters = source.char_indices().peekable();
    while let Some((byte, character)) = characters.next() {
        let point = Point {
            row,
            column: byte.saturating_sub(line_start),
        };
        let is_terminator = LINE_TERMINATORS.contains(&character);
        let is_irregular = IRREGULAR.contains(&character) && !(byte == 0 && character == BYTE_ORDER_MARK);
        if (is_terminator || (is_irregular && !in_run)) && !is_in_string(byte) {
            found.push(finding_at(
                LintKind::Eslint(EslintRule::NoIrregularWhitespace),
                point,
                "",
                None,
            ));
        }
        in_run = is_irregular;
        let ends_line = match character {
            '\n' => true,
            '\r' => characters.peek().is_none_or(|(_, next)| *next != '\n'),
            _ => is_terminator,
        };
        if ends_line {
            row = row.saturating_add(1);
            line_start = byte.saturating_add(character.len_utf8());
        }
    }
    found
}
