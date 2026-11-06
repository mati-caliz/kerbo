const BACKSLASH: char = '\\';
const CARRIAGE_RETURN: char = '\r';
const LINE_FEED: char = '\n';
const LINE_SEPARATOR: char = '\u{2028}';
const PARAGRAPH_SEPARATOR: char = '\u{2029}';
const HEX_RADIX: u32 = 16;
const OCTAL_RADIX: u32 = 8;
const LARGEST_LEGACY_OCTAL: u32 = 0o377;
const MAX_OCTAL_DIGITS: usize = 3;
const BYTE_ESCAPE_DIGITS: usize = 2;
const UNICODE_ESCAPE_DIGITS: usize = 4;

// Una unidad UTF-16 de un patrón, con dónde empieza en el texto del literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeUnit {
    pub value: u16,
    pub offset: usize,
}

// El cuerpo de un literal de regex: las unidades son las del texto tal cual.
pub fn from_pattern(pattern: &str, first_offset: usize) -> Vec<CodeUnit> {
    let mut units = Vec::new();
    for (index, character) in pattern.char_indices() {
        push_character(&mut units, character, first_offset.saturating_add(index));
    }
    units
}

// `parseStringLiteral` y `parseTemplateToken` de ESLint: `raw` es el literal sin sus comillas.
pub fn from_string_body(raw: &str, first_offset: usize) -> Vec<CodeUnit> {
    let mut units = Vec::new();
    let mut index = 0;
    while let Some(character) = raw.get(index..).and_then(|rest| rest.chars().next()) {
        let offset = first_offset.saturating_add(index);
        let after = index.saturating_add(character.len_utf8());
        if character != BACKSLASH {
            push_character(&mut units, character, offset);
            index = after;
            continue;
        }
        let rest = raw.get(after..).unwrap_or_default();
        let (values, consumed) = decode_escape(rest);
        units.extend(values.into_iter().map(|value| CodeUnit { value, offset }));
        index = after.saturating_add(consumed);
    }
    units
}

fn push_character(units: &mut Vec<CodeUnit>, character: char, offset: usize) {
    let mut buffer = [0_u16; 2];
    units.extend(character.encode_utf16(&mut buffer).iter().map(|value| CodeUnit {
        value: *value,
        offset,
    }));
}

fn decode_escape(rest: &str) -> (Vec<u16>, usize) {
    let Some(escaped) = rest.chars().next() else {
        return (Vec::new(), 0);
    };
    let width = escaped.len_utf8();
    let tail = rest.get(width..).unwrap_or_default();
    match escaped {
        'n' => (vec![0x0a], width),
        'r' => (vec![0x0d], width),
        't' => (vec![0x09], width),
        'b' => (vec![0x08], width),
        'f' => (vec![0x0c], width),
        'v' => (vec![0x0b], width),
        CARRIAGE_RETURN if tail.starts_with(LINE_FEED) => (Vec::new(), width.saturating_add(1)),
        CARRIAGE_RETURN | LINE_FEED | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => (Vec::new(), width),
        'x' => hex_escape(tail, BYTE_ESCAPE_DIGITS).unwrap_or_else(|| identity(escaped, width)),
        'u' => unicode_escape(tail).unwrap_or_else(|| identity(escaped, width)),
        '0'..='7' => legacy_octal(rest),
        _ => identity(escaped, width),
    }
}

fn identity(character: char, width: usize) -> (Vec<u16>, usize) {
    let mut buffer = [0_u16; 2];
    (character.encode_utf16(&mut buffer).to_vec(), width)
}

fn hex_escape(tail: &str, digits: usize) -> Option<(Vec<u16>, usize)> {
    let value = u16::from_str_radix(tail.get(..digits)?, HEX_RADIX).ok()?;
    Some((vec![value], digits.saturating_add(1)))
}

fn unicode_escape(tail: &str) -> Option<(Vec<u16>, usize)> {
    let Some(braced) = tail.strip_prefix('{') else {
        return hex_escape(tail, UNICODE_ESCAPE_DIGITS);
    };
    let (digits, _) = braced.split_once('}')?;
    let code_point = u32::from_str_radix(digits, HEX_RADIX).ok()?;
    let character = char::from_u32(code_point);
    let values = character.map_or_else(
        || u16::try_from(code_point).map_or_else(|_| Vec::new(), |unit| vec![unit]),
        |found| identity(found, 0).0,
    );
    Some((values, digits.len().saturating_add("u{}".len())))
}

fn legacy_octal(rest: &str) -> (Vec<u16>, usize) {
    let mut value = 0_u32;
    let mut consumed = 0_usize;
    for digit in rest
        .chars()
        .take(MAX_OCTAL_DIGITS)
        .map_while(|digit| digit.to_digit(OCTAL_RADIX))
    {
        let next = value.saturating_mul(OCTAL_RADIX).saturating_add(digit);
        if next > LARGEST_LEGACY_OCTAL {
            break;
        }
        value = next;
        consumed = consumed.saturating_add(1);
    }
    (
        u16::try_from(value).map_or_else(|_| Vec::new(), |unit| vec![unit]),
        consumed,
    )
}
