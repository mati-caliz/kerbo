const DIGIT_SEPARATOR: char = '_';
const HEX: u32 = 16;
const OCTAL: u32 = 8;
const BINARY: u32 = 2;
const DECIMAL: u32 = 10;
const RADIX_PREFIXES: &[(&str, u32)] = &[("0x", HEX), ("0o", OCTAL), ("0b", BINARY)];
const LEGACY_OCTAL_PREFIX: char = '0';
const INTEGER_SUFFIX: char = 'l';
const DECIMAL_SUFFIXES: &[char] = &['l', 'f', 'd'];

pub fn numeric_value(literal: &str) -> Option<f64> {
    let normalized = literal
        .chars()
        .filter(|&character| character != DIGIT_SEPARATOR)
        .collect::<String>()
        .to_ascii_lowercase();
    if let Some((digits, radix)) = RADIX_PREFIXES
        .iter()
        .find_map(|&(prefix, radix)| normalized.strip_prefix(prefix).map(|digits| (digits, radix)))
    {
        return integer_value(digits.trim_end_matches(INTEGER_SUFFIX), radix);
    }
    let decimal = normalized.trim_end_matches(DECIMAL_SUFFIXES);
    if is_legacy_octal(decimal) {
        return integer_value(decimal, OCTAL);
    }
    decimal.parse().ok()
}

fn is_legacy_octal(digits: &str) -> bool {
    digits.len() > 1
        && digits.starts_with(LEGACY_OCTAL_PREFIX)
        && digits.chars().all(|character| character.is_digit(OCTAL))
}

fn integer_value(digits: &str, radix: u32) -> Option<f64> {
    if digits.is_empty() {
        return None;
    }
    digits.chars().try_fold(0.0_f64, |value, character| {
        character
            .to_digit(radix)
            .map(|digit| value.mul_add(f64::from(radix), f64::from(digit)))
    })
}

pub fn java_integer_value(literal: &str) -> Option<i64> {
    let normalized = literal
        .chars()
        .filter(|&character| character != DIGIT_SEPARATOR)
        .collect::<String>()
        .to_ascii_lowercase();
    let is_long = normalized.ends_with(INTEGER_SUFFIX);
    let digits = normalized.trim_end_matches(INTEGER_SUFFIX);
    let (digits, radix) = RADIX_PREFIXES
        .iter()
        .find_map(|&(prefix, radix)| digits.strip_prefix(prefix).map(|rest| (rest, radix)))
        .unwrap_or_else(|| {
            let radix = if digits.starts_with(LEGACY_OCTAL_PREFIX) {
                OCTAL
            } else {
                DECIMAL
            };
            (digits, radix)
        });
    let unsigned = u64::from_str_radix(digits, radix).ok()?;
    if is_long {
        return Some(i64::from_ne_bytes(unsigned.to_ne_bytes()));
    }
    let narrow = u32::try_from(unsigned).ok()?;
    Some(i64::from(i32::from_ne_bytes(narrow.to_ne_bytes())))
}
