const DIGIT_SEPARATOR: char = '_';
const HEX: u32 = 16;
const OCTAL: u32 = 8;
const BINARY: u32 = 2;
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
