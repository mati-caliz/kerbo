mod token_kind;
mod tokenizer;

pub use token_kind::TokenKind;
pub use tokenizer::{BackwardsTokenizer, Token, Tokenizer};

const LINE_FEED: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const WINDOWS_LINE_ENDING: &str = "\r\n";
const SPACE: char = ' ';
const TAB: char = '\t';
const FORM_FEED: char = '\x0C';

pub const fn is_python_whitespace(character: char) -> bool {
    matches!(character, SPACE | TAB | FORM_FEED)
}

pub const fn is_line_break(character: char) -> bool {
    matches!(character, LINE_FEED | CARRIAGE_RETURN)
}

fn count_line_breaks(characters: impl Iterator<Item = char>, second_of_a_pair: char) -> usize {
    let mut newlines = 0_usize;
    let mut characters = characters.peekable();
    while let Some(character) = characters.next() {
        if is_line_break(character) {
            if character != second_of_a_pair {
                characters.next_if_eq(&second_of_a_pair);
            }
            newlines = newlines.saturating_add(1);
        } else if !is_python_whitespace(character) {
            break;
        }
    }
    newlines
}

// Los saltos de línea que hay después de `offset` antes del próximo carácter que no es espacio:
// `lines_after` de ruff_python_trivia.
pub fn lines_after(source: &str, offset: usize) -> usize {
    count_line_breaks(source.get(offset..).unwrap_or_default().chars(), LINE_FEED)
}

pub fn lines_before(source: &str, offset: usize) -> usize {
    count_line_breaks(
        source.get(..offset).unwrap_or_default().chars().rev(),
        CARRIAGE_RETURN,
    )
}

// Como `lines_after`, pero un comentario en el camino vuelve la cuenta a cero.
pub fn lines_after_ignoring_trivia(source: &str, offset: usize) -> usize {
    let mut newlines = 0_usize;
    for token in Tokenizer::starts_at(offset, source) {
        if token.kind == TokenKind::Newline {
            newlines = newlines.saturating_add(1);
        } else if token.kind == TokenKind::Comment {
            newlines = 0;
        } else if token.kind != TokenKind::Whitespace {
            break;
        }
    }
    newlines
}

// Los saltos de línea después del comentario de fin de línea que pueda haber tras `offset`.
pub fn lines_after_ignoring_end_of_line_trivia(source: &str, offset: usize) -> usize {
    Tokenizer::starts_at(offset, source)
        .skip_while(|token| token.kind != TokenKind::Newline && token.kind.is_trivia())
        .take_while(|token| matches!(token.kind, TokenKind::Newline | TokenKind::Whitespace))
        .filter(|token| token.kind == TokenKind::Newline)
        .count()
}

fn line_start(source: &str, offset: usize) -> usize {
    let before = source.get(..offset).unwrap_or_default();
    before
        .rfind(is_line_break)
        .map_or(0, |line_break| line_break.saturating_add(1))
}

// El final del renglón de `offset`, con su salto de línea.
pub fn full_line_end(source: &str, offset: usize) -> usize {
    let rest = source.get(offset..).unwrap_or_default();
    let Some(line_break) = rest.find(is_line_break) else {
        return source.len();
    };
    let after = rest.get(line_break..).unwrap_or_default();
    let length = if after.starts_with(WINDOWS_LINE_ENDING) {
        WINDOWS_LINE_ENDING.len()
    } else {
        1
    };
    offset.saturating_add(line_break).saturating_add(length)
}

// La sangría del renglón de `offset`, si antes de `offset` no hay otra cosa.
pub fn indentation_at_offset(source: &str, offset: usize) -> Option<&str> {
    let indentation = source.get(line_start(source, offset)..offset)?;
    indentation
        .chars()
        .all(is_python_whitespace)
        .then_some(indentation)
}

pub fn contains_line_break(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|text| text.contains(is_line_break))
}

pub fn first_non_trivia_token(source: &str, offset: usize) -> Option<Token> {
    Tokenizer::starts_at(offset, source).skip_trivia().next()
}

// ruff escribe con el salto de línea del primer renglón del archivo (`line-ending = "auto"`).
pub fn with_the_line_ending_of(source: &str, printed: &str) -> String {
    let Some(first) = source.find(is_line_break) else {
        return printed.to_owned();
    };
    let rest = source.get(first..).unwrap_or_default();
    if rest.starts_with(WINDOWS_LINE_ENDING) {
        printed.replace(LINE_FEED, WINDOWS_LINE_ENDING)
    } else if rest.starts_with(CARRIAGE_RETURN) {
        printed.replace(LINE_FEED, &CARRIAGE_RETURN.to_string())
    } else {
        printed.to_owned()
    }
}
