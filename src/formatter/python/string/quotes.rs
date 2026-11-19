use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

use super::flags::StringFlags;
use crate::formatter::python::context::Quote;

const BACKSLASH: char = '\\';

#[derive(Debug, Clone, Copy)]
enum QuoteMetadataKind {
    Raw { contains_preferred: bool },
    Triple { contains_preferred: bool },
    Regular { single_quotes: u32, double_quotes: u32 },
}

// Lo que hay que saber del contenido de una cadena para elegirle las comillas: cuántas tiene de
// cada tipo, o si las preferidas la cortarían.
#[derive(Debug, Clone, Copy)]
pub struct QuoteMetadata {
    kind: QuoteMetadataKind,
    source_style: Quote,
}

fn next_is(characters: &mut Peekable<Chars<'_>>, expected: char) -> bool {
    characters.peek() == Some(&expected)
}

// Si el contenido de una cadena de comillas triples tiene tres comillas preferidas seguidas, o
// termina con una o dos.
fn triple_quoted_contains(content: &str, preferred: char) -> bool {
    let mut characters = content.chars().peekable();
    while let Some(character) = characters.next() {
        if character == BACKSLASH {
            if matches!(characters.peek(), Some('"' | '\\')) {
                characters.next();
            }
            continue;
        }
        if character != preferred {
            continue;
        }
        if characters.peek().is_none() {
            return true;
        }
        if !next_is(&mut characters, preferred) {
            continue;
        }
        characters.next();
        if characters.peek().is_none() || next_is(&mut characters, preferred) {
            return true;
        }
    }
    false
}

// En una cadena cruda no se puede escapar una comilla: si tiene la preferida, se queda con las
// suyas.
fn raw_contains(text: &str, preferred: char, is_triple_quoted: bool) -> bool {
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == BACKSLASH {
            characters.next();
            continue;
        }
        if character != preferred {
            continue;
        }
        if !is_triple_quoted || characters.peek().is_none() {
            return true;
        }
        if !next_is(&mut characters, preferred) {
            continue;
        }
        characters.next();
        if characters.peek().is_none() || next_is(&mut characters, preferred) {
            return true;
        }
    }
    false
}

fn count_of(text: &str, quote: char) -> u32 {
    u32::try_from(text.chars().filter(|character| *character == quote).count()).unwrap_or(u32::MAX)
}

impl QuoteMetadata {
    pub fn from_str(text: &str, flags: StringFlags, preferred: Quote) -> Self {
        let kind = if flags.is_raw() {
            QuoteMetadataKind::Raw {
                contains_preferred: raw_contains(text, preferred.as_char(), flags.is_triple_quoted),
            }
        } else if flags.is_triple_quoted {
            QuoteMetadataKind::Triple {
                contains_preferred: triple_quoted_contains(text, preferred.as_char()),
            }
        } else {
            QuoteMetadataKind::Regular {
                single_quotes: count_of(text, Quote::Single.as_char()),
                double_quotes: count_of(text, Quote::Double.as_char()),
            }
        };
        Self {
            kind,
            source_style: flags.quote,
        }
    }

    // Las comillas que piden menos escapes; con empate, las preferidas.
    pub fn choose(self, preferred: Quote) -> Quote {
        match self.kind {
            QuoteMetadataKind::Raw { contains_preferred }
            | QuoteMetadataKind::Triple { contains_preferred } => {
                if contains_preferred {
                    self.source_style
                } else {
                    preferred
                }
            }
            QuoteMetadataKind::Regular {
                single_quotes,
                double_quotes,
            } => match single_quotes.cmp(&double_quotes) {
                Ordering::Less => Quote::Single,
                Ordering::Equal => preferred,
                Ordering::Greater => Quote::Double,
            },
        }
    }

    // Las partes de una concatenación implícita se juzgan juntas si son del mismo tipo.
    pub const fn merge(self, other: Self) -> Option<Self> {
        let kind = match (self.kind, other.kind) {
            (
                QuoteMetadataKind::Regular {
                    single_quotes: own_single,
                    double_quotes: own_double,
                },
                QuoteMetadataKind::Regular {
                    single_quotes: other_single,
                    double_quotes: other_double,
                },
            ) => QuoteMetadataKind::Regular {
                single_quotes: own_single.saturating_add(other_single),
                double_quotes: own_double.saturating_add(other_double),
            },
            (
                QuoteMetadataKind::Raw {
                    contains_preferred: own,
                },
                QuoteMetadataKind::Raw {
                    contains_preferred: other,
                },
            ) => QuoteMetadataKind::Raw {
                contains_preferred: own || other,
            },
            (
                QuoteMetadataKind::Triple {
                    contains_preferred: own,
                },
                QuoteMetadataKind::Triple {
                    contains_preferred: other,
                },
            ) => QuoteMetadataKind::Triple {
                contains_preferred: own || other,
            },
            (
                QuoteMetadataKind::Regular { .. }
                | QuoteMetadataKind::Raw { .. }
                | QuoteMetadataKind::Triple { .. },
                _,
            ) => {
                return None;
            }
        };
        Some(Self {
            kind,
            source_style: self.source_style,
        })
    }
}
