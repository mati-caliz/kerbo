use std::sync::OnceLock;

use regex::bytes::Regex;

use crate::secrets::catalog::compile;

const LOWEST_PRINTABLE: u8 = 0x09;
const HIGHEST_PRINTABLE: u8 = 0x7e;
const PERCENT: u8 = b'%';
const PERCENT_ESCAPE_LENGTH: usize = 3;
const HEX_RADIX: u32 = 16;
const HEX_PAIR_LENGTH: usize = 2;
const NIBBLE_BITS: u32 = 4;
const BASE64_PADDING: u8 = b'=';
const SEXTET_BITS: u32 = 6;
const BYTE_BITS: u32 = 8;
const BASE64_QUANTUM: usize = 4;
const LIKELY_BASE64: &[u8] = b"0123456789+/-_";
const STANDARD_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const CODE_POINT_PREFIX: &str = "U+";
const CODE_POINT_DIGITS: usize = 4;
const CODE_POINTS: &str = r"(?:U\+[a-fA-F0-9]{4}(?:\s|$))+";
const ESCAPES: &str = r"(?i)(?:\\{1,2}u[a-fA-F0-9]{4})+";
const ESCAPE: &str = r"(?i)\\{1,2}u([a-fA-F0-9]{4})";
type SequenceDecoder<'pattern> = Box<dyn Fn(&str) -> String + 'pattern>;
const ENCODINGS: &str = r"(%[0-9A-Fa-f]{2}(?:.*%[0-9A-Fa-f]{2})?)|((?:(?:U\+[a-fA-F0-9]{4}(?:\s|$))+|(?i)(?:\\{1,2}u[a-fA-F0-9]{4})+))|([0-9A-Fa-f]{32,})|([\w\/+-]{16,}={0,2})";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Percent,
    Unicode,
    Hex,
    Base64,
}

impl Encoding {
    pub const ALL: [Self; 4] = [Self::Percent, Self::Unicode, Self::Hex, Self::Base64];

    pub const fn precedence(self) -> usize {
        match self {
            Self::Percent => 4,
            Self::Unicode => 3,
            Self::Hex => 2,
            Self::Base64 => 1,
        }
    }

    pub fn decode(self, encoded: &[u8]) -> Vec<u8> {
        match self {
            Self::Percent => decode_percent(encoded),
            Self::Unicode => decode_unicode(encoded),
            Self::Hex => decode_hex(encoded),
            Self::Base64 => decode_base64(encoded),
        }
    }
}

pub fn encodings_pattern() -> Option<&'static Regex> {
    static PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    PATTERN.get_or_init(|| compile(ENCODINGS).ok()).as_ref()
}

const fn is_printable(byte: u8) -> bool {
    LOWEST_PRINTABLE <= byte && byte <= HIGHEST_PRINTABLE
}

fn hex_value(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(HEX_RADIX)
        .and_then(|digit| u8::try_from(digit).ok())
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    Some((hex_value(high)? << NIBBLE_BITS) | hex_value(low)?)
}

fn decode_percent(encoded: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut index = 0;
    while let Some(&byte) = encoded.get(index) {
        let escaped = if byte == PERCENT && index.saturating_add(PERCENT_ESCAPE_LENGTH) <= encoded.len() {
            encoded
                .get(index.saturating_add(1)..index.saturating_add(PERCENT_ESCAPE_LENGTH))
                .and_then(|pair| match pair {
                    [high, low] => hex_pair(*high, *low),
                    _ => None,
                })
        } else {
            None
        };
        if let Some(value) = escaped {
            if !is_printable(value) {
                return Vec::new();
            }
            decoded.push(value);
            index = index.saturating_add(PERCENT_ESCAPE_LENGTH);
            continue;
        }
        decoded.push(byte);
        index = index.saturating_add(1);
    }
    decoded
}

fn decode_hex(encoded: &[u8]) -> Vec<u8> {
    if !encoded.len().is_multiple_of(HEX_PAIR_LENGTH) || !encoded.iter().any(u8::is_ascii_digit) {
        return Vec::new();
    }
    let (pairs, _) = encoded.as_chunks::<HEX_PAIR_LENGTH>();
    let decoded: Option<Vec<u8>> = pairs
        .iter()
        .map(|[high, low]| hex_pair(*high, *low).filter(|&value| is_printable(value)))
        .collect();
    decoded.unwrap_or_default()
}

fn decode_base64(encoded: &[u8]) -> Vec<u8> {
    if !encoded.iter().any(|byte| LIKELY_BASE64.contains(byte)) {
        return Vec::new();
    }
    [
        base64(encoded, STANDARD_ALPHABET, true),
        base64(encoded, URL_ALPHABET, false),
    ]
    .into_iter()
    .flatten()
    .find(|decoded| decoded.iter().copied().all(is_printable))
    .unwrap_or_default()
}

fn base64(encoded: &[u8], alphabet: &[u8], padded: bool) -> Option<Vec<u8>> {
    let body = if padded {
        if !encoded.len().is_multiple_of(BASE64_QUANTUM) {
            return None;
        }
        let trimmed = encoded.strip_suffix(&[BASE64_PADDING, BASE64_PADDING]);
        trimmed
            .or_else(|| encoded.strip_suffix(&[BASE64_PADDING]))
            .unwrap_or(encoded)
    } else {
        encoded
    };
    if body.len() % BASE64_QUANTUM == 1 {
        return None;
    }
    let mut decoded = Vec::with_capacity(body.len());
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in body {
        let value = alphabet.iter().position(|known| known == byte)?;
        buffer = (buffer << SEXTET_BITS) | u32::try_from(value).ok()?;
        bits = bits.saturating_add(SEXTET_BITS);
        if bits >= BYTE_BITS {
            bits = bits.saturating_sub(BYTE_BITS);
            decoded.push(u8::try_from((buffer >> bits) & u32::from(u8::MAX)).ok()?);
        }
    }
    Some(decoded)
}

fn unicode_patterns() -> Option<&'static [Regex; 3]> {
    static PATTERNS: OnceLock<Option<[Regex; 3]>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            Some([
                compile(CODE_POINTS).ok()?,
                compile(ESCAPES).ok()?,
                compile(ESCAPE).ok()?,
            ])
        })
        .as_ref()
}

fn decode_unicode(encoded: &[u8]) -> Vec<u8> {
    let Some([code_points, escapes, escape]) = unicode_patterns() else {
        return encoded.to_vec();
    };
    let text = String::from_utf8_lossy(encoded).into_owned();
    let (pattern, decode): (&Regex, SequenceDecoder<'_>) = if code_points.is_match(encoded) {
        (code_points, Box::new(decode_code_points))
    } else if escapes.is_match(encoded) {
        (
            escapes,
            Box::new(|sequence: &str| decode_escapes(escape, sequence)),
        )
    } else {
        return encoded.to_vec();
    };
    let sequences: Vec<String> = pattern
        .find_iter(text.as_bytes())
        .map(|found| String::from_utf8_lossy(found.as_bytes()).into_owned())
        .collect();
    let replaced = sequences.iter().fold(text, |current, sequence| {
        let decoded = decode(sequence);
        if decoded.is_empty() || decoded == *sequence {
            current
        } else {
            current.replacen(sequence.as_str(), &decoded, 1)
        }
    });
    replaced.into_bytes()
}

fn code_point(hex: &str) -> char {
    u32::from_str_radix(hex, HEX_RADIX)
        .ok()
        .and_then(char::from_u32)
        .unwrap_or(char::REPLACEMENT_CHARACTER)
}

fn decode_code_points(sequence: &str) -> String {
    let decoded: String = sequence
        .split_whitespace()
        .filter_map(|point| point.strip_prefix(CODE_POINT_PREFIX))
        .filter(|hex| hex.len() >= CODE_POINT_DIGITS)
        .map(code_point)
        .collect();
    if decoded.is_empty() {
        sequence.to_owned()
    } else {
        decoded
    }
}

fn decode_escapes(escape: &Regex, sequence: &str) -> String {
    let decoded: String = escape
        .captures_iter(sequence.as_bytes())
        .filter_map(|captures| captures.get(1))
        .map(|hex| code_point(&String::from_utf8_lossy(hex.as_bytes())))
        .collect();
    if decoded.is_empty() {
        sequence.to_owned()
    } else {
        decoded
    }
}
