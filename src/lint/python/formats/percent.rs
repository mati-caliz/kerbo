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
    Unsupported,
    Invalid,
}

pub fn summary(format: &str) -> Result<PercentSummary, PercentError> {
    let mut summary = PercentSummary::default();
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != MARKER || characters.next_if_eq(&MARKER).is_some() {
            continue;
        }
        specifier(&mut characters, &mut summary)?;
    }
    Ok(summary)
}

fn specifier(characters: &mut Peekable<Chars<'_>>, summary: &mut PercentSummary) -> Result<(), PercentError> {
    let key = mapping_key(characters)?;
    while characters.next_if(|flag| FLAGS.contains(flag)).is_some() {}
    let mut stars = usize::from(quantity(characters));
    if characters.next_if_eq(&PRECISION).is_some() {
        stars = stars.saturating_add(usize::from(quantity(characters)));
    }
    characters.next_if(|modifier| LENGTH_MODIFIERS.contains(modifier));
    let conversion = characters.next().ok_or(PercentError::Invalid)?;
    if !CONVERSIONS.contains(&conversion) {
        return Err(PercentError::Unsupported);
    }
    match key {
        Some(key) => {
            summary.keywords.insert(key);
        }
        None => summary.positional = summary.positional.saturating_add(1),
    }
    summary.positional = summary.positional.saturating_add(stars);
    summary.starred |= stars > 0;
    Ok(())
}

fn quantity(characters: &mut Peekable<Chars<'_>>) -> bool {
    if characters.next_if_eq(&STAR).is_some() {
        return true;
    }
    while characters.next_if(char::is_ascii_digit).is_some() {}
    false
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
