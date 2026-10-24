use std::collections::BTreeSet;
use std::iter::Peekable;
use std::str::Chars;

const MARKER: char = '%';
const KEY_OPENING: char = '(';
const KEY_CLOSING: char = ')';
const STAR: char = '*';
const PRECISION: char = '.';
const FLAGS: [char; 5] = ['#', '0', '-', ' ', '+'];
const LENGTH_MODIFIERS: [char; 3] = ['h', 'l', 'L'];
const CONVERSIONS: [char; 16] = [
    'd', 'i', 'o', 'u', 'x', 'X', 'e', 'E', 'f', 'F', 'g', 'G', 'c', 'r', 's', 'a',
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PercentSummary {
    pub starred: bool,
    pub positional: usize,
    pub keywords: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentError {
    Unsupported(char),
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    Star,
    Digits,
}

// Un `%…` del formato: clave, flags, ancho, precisión y conversión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specifier {
    pub key: Option<String>,
    pub flagged: bool,
    pub width: Option<Quantity>,
    pub precision: Option<Quantity>,
    pub conversion: char,
}

impl Specifier {
    fn stars(&self) -> usize {
        [self.width, self.precision]
            .into_iter()
            .filter(|quantity| *quantity == Some(Quantity::Star))
            .count()
    }
}

pub fn specifiers(format: &str) -> Result<Vec<Specifier>, PercentError> {
    let mut found = Vec::new();
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != MARKER || characters.next_if_eq(&MARKER).is_some() {
            continue;
        }
        found.push(specifier(&mut characters)?);
    }
    Ok(found)
}

pub fn summary(format: &str) -> Result<PercentSummary, PercentError> {
    let mut summary = PercentSummary::default();
    for specifier in specifiers(format)? {
        let stars = specifier.stars();
        match specifier.key {
            Some(key) => {
                summary.keywords.insert(key);
            }
            None => summary.positional = summary.positional.saturating_add(1),
        }
        summary.positional = summary.positional.saturating_add(stars);
        summary.starred |= stars > 0;
    }
    Ok(summary)
}

fn specifier(characters: &mut Peekable<Chars<'_>>) -> Result<Specifier, PercentError> {
    let key = mapping_key(characters)?;
    let mut flagged = false;
    while characters.next_if(|flag| FLAGS.contains(flag)).is_some() {
        flagged = true;
    }
    let width = quantity(characters);
    let precision = characters
        .next_if_eq(&PRECISION)
        .map(|_| quantity(characters).unwrap_or(Quantity::Digits));
    characters.next_if(|modifier| LENGTH_MODIFIERS.contains(modifier));
    let conversion = characters.next().ok_or(PercentError::Invalid)?;
    if !CONVERSIONS.contains(&conversion) {
        return Err(PercentError::Unsupported(conversion));
    }
    Ok(Specifier {
        key,
        flagged,
        width,
        precision,
        conversion,
    })
}

fn quantity(characters: &mut Peekable<Chars<'_>>) -> Option<Quantity> {
    if characters.next_if_eq(&STAR).is_some() {
        return Some(Quantity::Star);
    }
    let mut digits = false;
    while characters.next_if(char::is_ascii_digit).is_some() {
        digits = true;
    }
    digits.then_some(Quantity::Digits)
}

fn mapping_key(characters: &mut Peekable<Chars<'_>>) -> Result<Option<String>, PercentError> {
    if characters.next_if_eq(&KEY_OPENING).is_none() {
        return Ok(None);
    }
    let mut depth: usize = 1;
    let mut key = String::new();
    for character in characters.by_ref() {
        if character == KEY_OPENING {
            depth = depth.saturating_add(1);
        } else if character == KEY_CLOSING {
            depth = depth.saturating_sub(1);
        }
        if depth == 0 {
            return Ok(Some(key));
        }
        key.push(character);
    }
    Err(PercentError::Invalid)
}
