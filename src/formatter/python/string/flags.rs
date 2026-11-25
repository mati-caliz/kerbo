use crate::formatter::python::ast::Span;
use crate::formatter::python::context::Quote;

const TRIPLE_QUOTE_LENGTH: usize = 3;
const SINGLE_QUOTE_LENGTH: usize = 1;
const QUOTES: [char; 2] = ['"', '\''];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPrefix {
    None,
    Lowercase,
    Uppercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Plain,
    Bytes,
    Format,
}

impl Family {
    fn of(prefix: &str) -> Self {
        if prefix.contains(['b', 'B']) {
            Self::Bytes
        } else if prefix.contains(['f', 'F', 't', 'T']) {
            Self::Format
        } else {
            Self::Plain
        }
    }
}

// El prefijo y las comillas de una parte de cadena, leídos del fuente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringFlags {
    pub raw: RawPrefix,
    pub family: Family,
    pub quote: Quote,
    pub is_triple_quoted: bool,
    pub prefix_length: usize,
}

impl StringFlags {
    pub fn of(text: &str) -> Self {
        let prefix_length = text.find(QUOTES).unwrap_or(0);
        let prefix = text.get(..prefix_length).unwrap_or_default();
        let quoted = text.get(prefix_length..).unwrap_or_default();
        let quote = if quoted.starts_with('\'') {
            Quote::Single
        } else {
            Quote::Double
        };
        let triple = quote.as_char().to_string().repeat(TRIPLE_QUOTE_LENGTH);
        Self {
            raw: if prefix.contains('R') {
                RawPrefix::Uppercase
            } else if prefix.contains('r') {
                RawPrefix::Lowercase
            } else {
                RawPrefix::None
            },
            family: Family::of(prefix),
            quote,
            is_triple_quoted: quoted.len() >= TRIPLE_QUOTE_LENGTH.saturating_mul(2)
                && quoted.starts_with(&triple),
            prefix_length,
        }
    }

    pub const fn is_bytes(self) -> bool {
        matches!(self.family, Family::Bytes)
    }

    pub const fn is_interpolated(self) -> bool {
        matches!(self.family, Family::Format)
    }

    pub const fn is_raw(self) -> bool {
        !matches!(self.raw, RawPrefix::None)
    }

    pub const fn quote_length(self) -> usize {
        if self.is_triple_quoted {
            TRIPLE_QUOTE_LENGTH
        } else {
            SINGLE_QUOTE_LENGTH
        }
    }

    // El tramo de lo que hay entre las comillas, dentro del tramo de la parte.
    pub const fn content_span(self, part: Span) -> Span {
        Span::new(
            part.start
                .saturating_add(self.prefix_length)
                .saturating_add(self.quote_length()),
            part.end.saturating_sub(self.quote_length()),
        )
    }

    #[must_use]
    pub const fn with_quote(self, quote: Quote) -> Self {
        Self { quote, ..self }
    }

    // El prefijo como lo escribe ruff: `u` desaparece, `b` va en minúscula y después de la `r`,
    // que conserva su mayúscula.
    pub const fn prefix(self) -> &'static str {
        match (self.raw, self.family) {
            (RawPrefix::None, Family::Plain) => "",
            (RawPrefix::None, Family::Bytes) => "b",
            (RawPrefix::Lowercase, Family::Plain) => "r",
            (RawPrefix::Uppercase, Family::Plain) => "R",
            (RawPrefix::Lowercase, Family::Bytes) => "rb",
            (RawPrefix::Uppercase, Family::Bytes) => "Rb",
            (RawPrefix::None, Family::Format) => "f",
            (RawPrefix::Lowercase, Family::Format) => "rf",
            (RawPrefix::Uppercase, Family::Format) => "Rf",
        }
    }

    pub const fn quotes(self) -> &'static str {
        match (self.quote, self.is_triple_quoted) {
            (Quote::Single, false) => "'",
            (Quote::Single, true) => "'''",
            (Quote::Double, false) => "\"",
            (Quote::Double, true) => "\"\"\"",
        }
    }
}
