use std::num::FpCategory;

use tree_sitter::Node;

use super::nodes::{NUMBER, STRING, TEMPLATE_STRING, TEMPLATE_SUBSTITUTION, text};

const BACKSLASH: char = '\\';
const HEX_RADIX: u32 = 16;
const OCTAL_RADIX: u32 = 8;
const BINARY_RADIX: u32 = 2;
const DECIMAL_RADIX: u32 = 10;
const HEX_ESCAPE_DIGITS: usize = 2;
const UNICODE_ESCAPE_DIGITS: usize = 4;
const MAX_LEGACY_OCTAL_DIGITS: usize = 3;
const LARGEST_SINGLE_OCTAL_LEAD: char = '3';
const DELIMITER_BYTES: usize = 1;
const NUMERIC_SEPARATOR: char = '_';
const BIGINT_SUFFIX: char = 'n';
const MAX_PLAIN_EXPONENT: i32 = 21;
const MIN_PLAIN_EXPONENT: i32 = -6;
const LINE_SEPARATOR: char = '\u{2028}';
const PARAGRAPH_SEPARATOR: char = '\u{2029}';

pub fn static_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        STRING => Some(cooked(inner_text(node, source))),
        TEMPLATE_STRING if !has_substitutions(node) => Some(cooked(inner_text(node, source))),
        _ => None,
    }
}

pub fn has_substitutions(template: Node<'_>) -> bool {
    let mut cursor = template.walk();
    template
        .named_children(&mut cursor)
        .any(|child| child.kind() == TEMPLATE_SUBSTITUTION)
}

pub fn number_key(node: Node<'_>, source: &[u8]) -> Option<String> {
    (node.kind() == NUMBER).then(|| numeric_text(text(node, source)))?
}

fn inner_text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    let raw = text(node, source);
    let end = raw.len().saturating_sub(DELIMITER_BYTES);
    raw.get(DELIMITER_BYTES..end).unwrap_or_default()
}

pub fn cooked(raw: &str) -> String {
    let mut value = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((before, after)) = rest.split_once(BACKSLASH) {
        value.push_str(before);
        let mut characters = after.chars();
        let Some(escaped) = characters.next() else {
            return value;
        };
        let tail = characters.as_str();
        let (decoded, consumed) = escape_value(escaped, tail);
        value.extend(decoded);
        rest = tail.get(consumed..).unwrap_or_default();
    }
    value.push_str(rest);
    value
}

fn escape_value(escaped: char, rest: &str) -> (Option<char>, usize) {
    match escaped {
        'n' => (Some('\n'), 0),
        'r' => (Some('\r'), 0),
        't' => (Some('\t'), 0),
        'b' => (Some('\u{8}'), 0),
        'f' => (Some('\u{c}'), 0),
        'v' => (Some('\u{b}'), 0),
        '\r' => (None, usize::from(rest.starts_with('\n'))),
        '\n' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => (None, 0),
        'x' => hex_escape(rest, HEX_ESCAPE_DIGITS).unwrap_or((Some(escaped), 0)),
        'u' => unicode_escape(rest).unwrap_or((Some(escaped), 0)),
        '0'..='7' => legacy_octal(escaped, rest),
        other => (Some(other), 0),
    }
}

fn hex_escape(rest: &str, digits: usize) -> Option<(Option<char>, usize)> {
    let hex = rest.get(..digits)?;
    let code = u32::from_str_radix(hex, HEX_RADIX).ok()?;
    Some((char::from_u32(code), digits))
}

fn unicode_escape(rest: &str) -> Option<(Option<char>, usize)> {
    if let Some(braced) = rest.strip_prefix('{') {
        let end = braced.find('}')?;
        let code = u32::from_str_radix(braced.get(..end)?, HEX_RADIX).ok()?;
        return Some((char::from_u32(code), end.saturating_add(2)));
    }
    hex_escape(rest, UNICODE_ESCAPE_DIGITS)
}

fn legacy_octal(first: char, rest: &str) -> (Option<char>, usize) {
    let limit = if first <= LARGEST_SINGLE_OCTAL_LEAD {
        MAX_LEGACY_OCTAL_DIGITS.saturating_sub(1)
    } else {
        1
    };
    let following = rest
        .chars()
        .take(limit)
        .take_while(|digit| digit.is_digit(OCTAL_RADIX))
        .count();
    let digits: String = std::iter::once(first)
        .chain(rest.chars().take(following))
        .collect();
    let code = u32::from_str_radix(&digits, OCTAL_RADIX).unwrap_or_default();
    (char::from_u32(code), following)
}

pub fn numeric_text(raw: &str) -> Option<String> {
    let digits: String = raw
        .chars()
        .filter(|character| *character != NUMERIC_SEPARATOR)
        .collect();
    let lowercase = digits.to_ascii_lowercase();
    if let Some(bigint) = lowercase.strip_suffix(BIGINT_SUFFIX) {
        return integer_value(bigint).map(|value| value.to_string());
    }
    if let Some(value) = integer_value(&lowercase) {
        return Some(javascript_number(value_as_float(value)));
    }
    lowercase.parse::<f64>().ok().map(javascript_number)
}

fn integer_value(text: &str) -> Option<u128> {
    let (digits, radix) = if let Some(hex) = text.strip_prefix("0x") {
        (hex, HEX_RADIX)
    } else if let Some(octal) = text.strip_prefix("0o") {
        (octal, OCTAL_RADIX)
    } else if let Some(binary) = text.strip_prefix("0b") {
        (binary, BINARY_RADIX)
    } else if text.len() > 1 && text.starts_with('0') && text.chars().all(|digit| digit.is_digit(OCTAL_RADIX))
    {
        (text.get(1..)?, OCTAL_RADIX)
    } else if !text.is_empty() && text.chars().all(|digit| digit.is_ascii_digit()) {
        (text, DECIMAL_RADIX)
    } else {
        return None;
    };
    u128::from_str_radix(digits, radix).ok()
}

fn value_as_float(value: u128) -> f64 {
    value.to_string().parse().unwrap_or(f64::INFINITY)
}

pub fn javascript_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return "Infinity".to_owned();
    }
    if value.classify() == FpCategory::Zero {
        return "0".to_owned();
    }
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or_default();
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let point = exponent.saturating_add(1);
    let count = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    if count <= point && point <= MAX_PLAIN_EXPONENT {
        let zeros = usize::try_from(point.saturating_sub(count)).unwrap_or_default();
        return format!("{digits}{}", "0".repeat(zeros));
    }
    if 0 < point && point <= MAX_PLAIN_EXPONENT {
        let split = usize::try_from(point).unwrap_or_default();
        let (whole, fraction) = digits.split_at(split.min(digits.len()));
        return format!("{whole}.{fraction}");
    }
    if MIN_PLAIN_EXPONENT < point && point <= 0 {
        let zeros = usize::try_from(point.saturating_neg()).unwrap_or_default();
        return format!("0.{}{digits}", "0".repeat(zeros));
    }
    let sign = if point > 0 { "+" } else { "-" };
    let shown = point.saturating_sub(1).unsigned_abs();
    let (first, others) = digits.split_at(1.min(digits.len()));
    if others.is_empty() {
        format!("{first}e{sign}{shown}")
    } else {
        format!("{first}.{others}e{sign}{shown}")
    }
}

#[cfg(test)]
mod tests {
    use super::{cooked, numeric_text};

    #[test]
    fn strings_cook_like_javascript() {
        assert_eq!(cooked(r"a\nb\tc\x41B\u{43}\0\101\8\-"), "a\nb\tcABC\0A8-");
        assert_eq!(cooked("line \\\ncontinued \\\r\nagain"), "line continued again");
        assert_eq!(cooked(r"\b\f\v\r\'"), "\u{8}\u{c}\u{b}\r'");
        assert_eq!(cooked(r"\x\u{zz}\u12"), "xu{zz}u12");
        assert_eq!(cooked("end\\"), "end");
    }

    #[test]
    fn numbers_print_like_javascript() {
        let cases = [
            ("1", "1"),
            ("1.0", "1"),
            ("0x1F", "31"),
            ("0o17", "15"),
            ("0b101", "5"),
            ("017", "15"),
            ("08", "8"),
            ("1_000", "1000"),
            ("10n", "10"),
            (".5", "0.5"),
            ("1e3", "1000"),
            ("1e21", "1e+21"),
            ("1.5e21", "1.5e+21"),
            ("1e-7", "1e-7"),
            ("0.000001", "0.000001"),
            ("123.456", "123.456"),
            ("0", "0"),
        ];
        for (raw, shown) in cases {
            assert_eq!(numeric_text(raw).as_deref(), Some(shown), "{raw}");
        }
    }
}
