use std::borrow::Cow;

use super::flags::StringFlags;
use super::quotes::QuoteMetadata;
use crate::formatter::python::ast::NodeId;
use crate::formatter::python::context::{Formatter, Quote};

const BACKSLASH: char = '\\';
const CARRIAGE_RETURN: char = '\r';
const LINE_FEED: char = '\n';
const NEEDS_A_LOOK: [char; 4] = ['\\', '"', '\'', '\r'];
const HEX_ESCAPE_LENGTH: usize = 2;
const SHORT_UNICODE_ESCAPE_LENGTH: usize = 4;
const LONG_UNICODE_ESCAPE_LENGTH: usize = 8;
const SHORTEST_CHARACTER_NAME: usize = 3;

#[derive(Debug)]
pub struct NormalizedString<'source> {
    pub flags: StringFlags,
    pub text: Cow<'source, str>,
}

// Las comillas preferidas para una parte: dobles, salvo adentro de un docstring, donde una cadena
// de comillas triples usa las contrarias a las del docstring.
pub const fn preferred_quote(f: &Formatter<'_>, flags: StringFlags) -> Quote {
    match f.docstring_quote {
        Some(quote) if flags.is_triple_quoted => quote.opposite(),
        Some(_) | None => Quote::Double,
    }
}

pub fn first_quote_or_normalized_char_offset(raw_content: &str) -> Option<usize> {
    raw_content.find(NEEDS_A_LOOK)
}

pub fn choose_quotes(raw_content: &str, flags: StringFlags, preferred: Quote) -> StringFlags {
    let quote = first_quote_or_normalized_char_offset(raw_content).map_or(preferred, |offset| {
        let rest = raw_content.get(offset..).unwrap_or_default();
        QuoteMetadata::from_str(rest, flags, preferred).choose(preferred)
    });
    flags.with_quote(quote)
}

// Una parte de cadena con sus comillas elegidas y el contenido reescrito para esas comillas.
pub fn normalize<'source>(f: &Formatter<'source>, part: NodeId) -> NormalizedString<'source> {
    let source_flags = StringFlags::of(f.node_text(part));
    let raw_content = f.text_of(source_flags.content_span(f.ast.span(part)));
    let flags = choose_quotes(raw_content, source_flags, preferred_quote(f, source_flags));
    let text = first_quote_or_normalized_char_offset(raw_content)
        .map_or(Cow::Borrowed(raw_content), |offset| {
            normalize_string(raw_content, offset, flags)
        });
    NormalizedString { flags, text }
}

pub fn fmt_normalized(f: &mut Formatter<'_>, normalized: &NormalizedString<'_>) {
    f.token(normalized.flags.prefix());
    f.token(normalized.flags.quotes());
    f.text(&normalized.text);
    f.token(normalized.flags.quotes());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnicodeEscape {
    Hex(usize),
    CharacterName,
}

impl UnicodeEscape {
    const fn new(first: char, allow_unicode: bool) -> Option<Self> {
        match (first, allow_unicode) {
            ('x', _) => Some(Self::Hex(HEX_ESCAPE_LENGTH)),
            ('u', true) => Some(Self::Hex(SHORT_UNICODE_ESCAPE_LENGTH)),
            ('U', true) => Some(Self::Hex(LONG_UNICODE_ESCAPE_LENGTH)),
            ('N', true) => Some(Self::CharacterName),
            _ => None,
        }
    }

    // `\xAB` pasa a `\xab` y `\N{bullet}` a `\N{BULLET}`. `None` si no es un escape válido.
    fn normalize(self, input: &str) -> Option<String> {
        match self {
            Self::Hex(length) => {
                let digits = input.get(..length)?;
                digits
                    .chars()
                    .all(|digit| digit.is_ascii_hexdigit())
                    .then(|| digits.to_ascii_lowercase())
            }
            Self::CharacterName => {
                let name_end = input.find('}')?;
                let name = input.strip_prefix('{')?.get(..name_end.saturating_sub(1))?;
                let is_valid = name_end >= SHORTEST_CHARACTER_NAME
                    && name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '-'));
                is_valid.then(|| format!("{{{}}}", name.to_ascii_uppercase()))
            }
        }
    }
}

#[derive(Debug)]
struct Rewriter<'input> {
    input: &'input str,
    output: String,
    last_index: usize,
    flags: StringFlags,
}

impl Rewriter<'_> {
    fn copy_up_to(&mut self, index: usize) {
        self.output
            .push_str(self.input.get(self.last_index..index).unwrap_or_default());
        self.last_index = index;
    }

    // Lo que sigue a una barra invertida. Devuelve cuántos bytes más saltea el recorrido.
    fn escape(&mut self, index: usize, next: char) -> usize {
        let after_escape_start = index
            .saturating_add(BACKSLASH.len_utf8())
            .saturating_add(next.len_utf8());
        if next == BACKSLASH {
            return next.len_utf8();
        }
        let rest = self.input.get(after_escape_start..).unwrap_or_default();
        let normalized =
            UnicodeEscape::new(next, !self.flags.is_bytes).and_then(|escape| escape.normalize(rest));
        if let Some(normalized) = normalized {
            let escape_end = after_escape_start.saturating_add(normalized.len());
            if self.input.get(after_escape_start..escape_end) != Some(normalized.as_str()) {
                self.copy_up_to(after_escape_start);
                self.output.push_str(&normalized);
                self.last_index = escape_end;
            }
            return next.len_utf8().saturating_add(normalized.len());
        }
        if self.flags.is_triple_quoted {
            return 0;
        }
        if next == self.flags.quote.opposite().as_char() {
            self.copy_up_to(index);
            self.last_index = index.saturating_add(BACKSLASH.len_utf8());
            return next.len_utf8();
        }
        if next == self.flags.quote.as_char() {
            return next.len_utf8();
        }
        0
    }

    fn carriage_return(&mut self, index: usize, next: Option<char>) -> usize {
        self.copy_up_to(index);
        self.last_index = index.saturating_add(CARRIAGE_RETURN.len_utf8());
        if next == Some(LINE_FEED) {
            return 0;
        }
        self.output.push(LINE_FEED);
        0
    }

    fn unescaped_preferred_quote(&mut self, index: usize, quote: char) {
        self.copy_up_to(index);
        self.output.push(BACKSLASH);
        self.output.push(quote);
        self.last_index = index.saturating_add(quote.len_utf8());
    }
}

// Reescribe el contenido para las comillas nuevas: escapa las preferidas, desescapa las otras,
// normaliza los escapes hexadecimales y unicode, y pasa `\r\n` y `\r` a `\n`.
pub fn normalize_string(input: &str, start_offset: usize, flags: StringFlags) -> Cow<'_, str> {
    let mut rewriter = Rewriter {
        input,
        output: String::new(),
        last_index: 0,
        flags,
    };
    let preferred_quote = flags.quote.as_char();
    let mut skip_until = start_offset;
    for (relative_index, character) in input.get(start_offset..).unwrap_or_default().char_indices() {
        let index = start_offset.saturating_add(relative_index);
        if index < skip_until {
            continue;
        }
        let after = index.saturating_add(character.len_utf8());
        let next = input.get(after..).and_then(|rest| rest.chars().next());
        let skipped = if character == CARRIAGE_RETURN {
            rewriter.carriage_return(index, next)
        } else if flags.is_raw() {
            0
        } else if character == BACKSLASH {
            next.map_or(0, |next| rewriter.escape(index, next))
        } else {
            if !flags.is_triple_quoted && character == preferred_quote {
                rewriter.unescaped_preferred_quote(index, character);
            }
            0
        };
        skip_until = after.saturating_add(skipped);
    }
    if rewriter.last_index == 0 {
        return Cow::Borrowed(input);
    }
    let end = input.len();
    rewriter.copy_up_to(end);
    Cow::Owned(rewriter.output)
}
