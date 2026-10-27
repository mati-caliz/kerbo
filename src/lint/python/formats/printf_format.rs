const MARKER: char = '%';
const KEY_OPENING: char = '(';
const KEY_CLOSING: char = ')';
const STAR: char = '*';
const PRECISION: char = '.';
const LENGTH_MODIFIERS: [char; 3] = ['h', 'l', 'L'];
const CONVERSIONS: [char; 16] = [
    'd', 'i', 'o', 'u', 'x', 'X', 'e', 'E', 'f', 'F', 'g', 'G', 'c', 'r', 's', 'a',
];
const PLAIN_CONVERSIONS: [char; 3] = ['s', 'r', 'a'];
const SHOWN_CONVERSIONS: [char; 2] = ['r', 'a'];
const INTEGER_PRESENTATIONS: [char; 7] = ['b', 'c', 'd', 'o', 'x', 'X', 'n'];
const NAMED_ESCAPE: &str = "\\N{";
const ALTERNATE: u8 = 1;
const ZERO: u8 = 1 << 1;
const LEFT: u8 = 1 << 2;
const BLANK: u8 = 1 << 3;
const SIGN: u8 = 1 << 4;

#[derive(Debug, Default, Clone, Copy)]
struct Flags(u8);

impl Flags {
    const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

#[derive(Debug, Clone, Copy)]
enum Amount {
    Missing,
    Given(usize),
}

#[derive(Debug, Clone, Copy)]
enum Precision {
    Amount(usize),
    Dot,
}

#[derive(Debug)]
pub struct Spec {
    key: Option<String>,
    flags: Flags,
    width: Option<usize>,
    precision: Option<Precision>,
    conversion: char,
}

impl Spec {
    pub const fn is_named(&self) -> bool {
        self.key.is_some()
    }
}

#[derive(Debug)]
pub enum Part {
    Literal(String),
    Spec(Spec),
}

// El `CFormatString::parse` de ruff: `%%` queda como `%` en el texto y cada `%…` es una conversión.
pub fn parse(format: &str) -> Option<Vec<Part>> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != MARKER {
            literal.push(character);
            continue;
        }
        if characters.next_if_eq(&MARKER).is_some() {
            literal.push(MARKER);
            continue;
        }
        characters.peek()?;
        if !literal.is_empty() {
            parts.push(Part::Literal(std::mem::take(&mut literal)));
        }
        parts.push(Part::Spec(spec(&mut characters)?));
    }
    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    Some(parts)
}

fn spec(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<Spec> {
    let key = if characters.next_if_eq(&KEY_OPENING).is_some() {
        Some(key_text(characters)?)
    } else {
        None
    };
    let mut flags = Flags::default();
    while let Some(flag) = characters.peek().copied() {
        match flag {
            '#' => flags.0 |= ALTERNATE,
            '0' => flags.0 |= ZERO,
            '-' => flags.0 |= LEFT,
            ' ' => flags.0 |= BLANK,
            '+' => flags.0 |= SIGN,
            _ => break,
        }
        characters.next();
    }
    let width = match quantity(characters)? {
        Amount::Given(width) => Some(width),
        Amount::Missing => None,
    };
    let precision = if characters.next_if_eq(&PRECISION).is_some() {
        Some(match quantity(characters)? {
            Amount::Given(amount) => Precision::Amount(amount),
            Amount::Missing => Precision::Dot,
        })
    } else {
        None
    };
    characters.next_if(|character| LENGTH_MODIFIERS.contains(character));
    let conversion = characters
        .next()
        .filter(|character| CONVERSIONS.contains(character))?;
    Some(Spec {
        key,
        flags,
        width,
        precision,
        conversion,
    })
}

fn key_text(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let mut depth = 1_usize;
    let mut key = String::new();
    loop {
        let character = characters.next()?;
        if character == KEY_OPENING {
            depth = depth.saturating_add(1);
        } else if character == KEY_CLOSING {
            depth = depth.saturating_sub(1);
        }
        if depth == 0 {
            return Some(key);
        }
        key.push(character);
    }
}

// `*` (sacar el ancho de los valores) no se convierte: ya lo descarta la detección.
fn quantity(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<Amount> {
    if characters.peek() == Some(&STAR) {
        return None;
    }
    let mut amount: Option<i32> = None;
    while let Some(digit) = characters.peek().and_then(|character| character.to_digit(10)) {
        let digit = i32::try_from(digit).ok()?;
        amount = Some(amount.unwrap_or(0).checked_mul(10)?.checked_add(digit)?);
        characters.next();
    }
    Some(
        amount
            .and_then(|amount| usize::try_from(amount).ok())
            .map_or(Amount::Missing, Amount::Given),
    )
}

// El `handle_part` de ruff.
pub fn converted(part: &Part) -> String {
    match part {
        Part::Literal(literal) => curly_escape(literal),
        Part::Spec(spec) => converted_spec(spec),
    }
}

fn converted_spec(spec: &Spec) -> String {
    let mut format = String::from("{");
    if let Some(key) = &spec.key {
        format.push_str(key);
    }
    let flagged = spec.flags.0 != 0;
    let plain = PLAIN_CONVERSIONS.contains(&spec.conversion);
    if flagged || spec.width.is_some() || spec.precision.is_some() || !plain {
        format.push(':');
        format.push_str(&simplified_flags(spec.flags));
        if let Some(width) = spec.width {
            format.push_str(&width.to_string());
        }
        match spec.precision {
            Some(Precision::Amount(amount)) if INTEGER_PRESENTATIONS.contains(&spec.conversion) => {
                format.push('0');
                format.push_str(&amount.to_string());
            }
            Some(Precision::Amount(amount)) => {
                format.push(PRECISION);
                format.push_str(&amount.to_string());
            }
            Some(Precision::Dot) => format.push_str(".0"),
            None => {}
        }
    }
    if !plain {
        format.push(spec.conversion);
    }
    if SHOWN_CONVERSIONS.contains(&spec.conversion) {
        format.push('!');
        format.push(spec.conversion);
    }
    format.push('}');
    format
}

fn simplified_flags(flags: Flags) -> String {
    let mut simplified = String::new();
    if flags.has(LEFT) {
        simplified.push('<');
    }
    if flags.has(SIGN) {
        simplified.push('+');
    }
    if flags.has(ALTERNATE) {
        simplified.push('#');
    }
    if flags.has(BLANK) && !flags.has(SIGN) {
        simplified.push(' ');
    }
    if flags.has(ZERO) && !flags.has(LEFT) {
        simplified.push('0');
    }
    simplified
}

// El `curly_escape` de ruff: duplica las llaves salvo las de un `\N{NOMBRE}`.
pub fn curly_escape(literal: &str) -> String {
    let mut escaped = String::with_capacity(literal.len());
    let mut rest = literal;
    while let Some(character) = rest.chars().next() {
        if rest.starts_with(NAMED_ESCAPE)
            && let Some(close) = rest.get(NAMED_ESCAPE.len()..).and_then(|after| after.find('}'))
            && close > 0
        {
            let end = NAMED_ESCAPE.len().saturating_add(close).saturating_add(1);
            escaped.push_str(rest.get(..end).unwrap_or_default());
            rest = rest.get(end..).unwrap_or_default();
            continue;
        }
        if character == '{' || character == '}' {
            escaped.push(character);
        }
        escaped.push(character);
        rest = rest.get(character.len_utf8()..).unwrap_or_default();
    }
    escaped
}
