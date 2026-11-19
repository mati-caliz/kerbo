use std::borrow::Cow;

const HEX_MARKERS: [char; 2] = ['x', 'X'];
const UPPERCASE_BASE_MARKERS: [char; 3] = ['B', 'O', 'X'];
const EXPONENT_MARKERS: [char; 2] = ['e', 'E'];
const DOT: char = '.';
const ZERO: char = '0';

// `0XABcd` pasa a `0xABCD`: el prefijo de la base en minúscula y los dígitos hexadecimales en
// mayúscula.
pub fn normalize_integer(input: &str) -> Cow<'_, str> {
    let mut characters = input.chars();
    let (Some(ZERO), Some(marker)) = (characters.next(), characters.next()) else {
        return Cow::Borrowed(input);
    };
    let is_hex = HEX_MARKERS.contains(&marker);
    let digits = characters.as_str();
    let has_lowercase_hex_digits = is_hex && digits.contains(|digit: char| matches!(digit, 'a'..='f'));
    if !UPPERCASE_BASE_MARKERS.contains(&marker) && !has_lowercase_hex_digits {
        return Cow::Borrowed(input);
    }
    let mut output = String::with_capacity(input.len());
    output.push(ZERO);
    output.push(marker.to_ascii_lowercase());
    if is_hex {
        output.extend(digits.chars().map(|digit| digit.to_ascii_uppercase()));
    } else {
        output.push_str(digits);
    }
    Cow::Owned(output)
}

fn normalize_mantissa(mantissa: &str) -> String {
    let mut output = String::with_capacity(mantissa.len().saturating_add(2));
    if mantissa.starts_with(DOT) {
        output.push(ZERO);
    }
    output.push_str(mantissa);
    if mantissa.ends_with(DOT) {
        output.push(ZERO);
    }
    output
}

// `.5` pasa a `0.5`, `1.` a `1.0`, `1E+5` a `1e5`.
pub fn normalize_floating_number(input: &str) -> Cow<'_, str> {
    let (mantissa, exponent) = input.find(EXPONENT_MARKERS).map_or((input, None), |position| {
        (
            input.get(..position).unwrap_or_default(),
            input.get(position.saturating_add(1)..),
        )
    });
    let mut output = normalize_mantissa(mantissa);
    if let Some(exponent) = exponent {
        output.push('e');
        output.push_str(exponent.strip_prefix('+').unwrap_or(exponent));
    }
    if output == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(output)
    }
}
