use super::nodes::NUMBER;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const SEPARATOR: char = '_';
const BIGINT_SUFFIX: char = 'n';
const NON_DECIMAL_PREFIXES: &[(&str, u32)] = &[("0x", 4), ("0b", 1), ("0o", 3)];
const LEGACY_OCTAL_BITS: u32 = 3;
const SIGNIFICAND_BITS: usize = 53;
const EXPONENT_LIMIT_BITS: usize = 1024;
const MAX_REQUESTED_PRECISION: usize = 100;
const EXPONENT_MARKER: char = 'e';
const DECIMAL_POINT: char = '.';
const ZERO: char = '0';

// Un número en notación científica normalizada: los dígitos significativos y la magnitud.
#[derive(Debug, PartialEq, Eq)]
struct Scientific {
    coefficient: String,
    magnitude: i64,
}

// `no-loss-of-precision`: un literal numérico que el `double` de JavaScript no puede guardar tal cual.
pub fn precision_losses(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[NUMBER])
        .filter(|number| {
            let raw = context.text(*number);
            !raw.ends_with(BIGINT_SUFFIX) && loses_precision(raw)
        })
        .map(|number| report(EslintRule::NoLossOfPrecision, number, context.text(number)))
        .collect()
}

fn loses_precision(raw: &str) -> bool {
    let digits: String = raw.chars().filter(|character| *character != SEPARATOR).collect();
    let lowercase = digits.to_ascii_lowercase();
    if let Some((rest, bits)) = NON_DECIMAL_PREFIXES
        .iter()
        .find_map(|(prefix, bits)| lowercase.strip_prefix(prefix).map(|rest| (rest, *bits)))
    {
        return non_decimal_loses_precision(rest, bits);
    }
    if is_legacy_octal(&lowercase) {
        return non_decimal_loses_precision(&lowercase, LEGACY_OCTAL_BITS);
    }
    decimal_loses_precision(&lowercase)
}

fn is_legacy_octal(raw: &str) -> bool {
    raw.len() > 1 && raw.starts_with(ZERO) && raw.chars().all(|digit| digit.is_digit(8))
}

// Un entero en otra base es exacto si sus bits significativos entran en la mantisa.
fn non_decimal_loses_precision(digits: &str, bits_per_digit: u32) -> bool {
    let width: usize = bits_per_digit.try_into().unwrap_or(1);
    let binary = digits
        .chars()
        .filter_map(|digit| digit.to_digit(1 << bits_per_digit))
        .flat_map(|value| {
            (0..width)
                .rev()
                .map(move |bit| if (value >> bit) & 1 == 1 { '1' } else { ZERO })
        })
        .collect::<String>();
    let trimmed = binary.trim_start_matches(ZERO);
    let significant = trimmed.trim_end_matches(ZERO);
    trimmed.len() > EXPONENT_LIMIT_BITS || significant.len() > SIGNIFICAND_BITS
}

// `baseTenLosesPrecision`: los dígitos pedidos contra los que guarda el `double` con esa precisión.
fn decimal_loses_precision(raw: &str) -> bool {
    let cleaned = without_trailing_point(raw);
    let Ok(value) = cleaned.parse::<f64>() else {
        return false;
    };
    let requested = to_scientific(&cleaned, false);
    if value == 0.0 {
        return !requested.coefficient.chars().all(|digit| digit == ZERO);
    }
    let precision = requested.coefficient.len();
    if precision > MAX_REQUESTED_PRECISION || value.is_infinite() {
        return true;
    }
    let stored = format!("{value:.precision$e}", precision = precision.saturating_sub(1));
    requested != to_scientific(&stored, true)
}

// El `.replace(/\.(?=e|$)/, "")` de ESLint: un punto justo antes del exponente o al final.
fn without_trailing_point(raw: &str) -> String {
    let (mantissa, exponent) = raw
        .split_once(EXPONENT_MARKER)
        .map_or((raw, None), |(mantissa, exponent)| (mantissa, Some(exponent)));
    let mantissa = mantissa.strip_suffix(DECIMAL_POINT).unwrap_or(mantissa);
    exponent.map_or_else(
        || mantissa.to_owned(),
        |exponent| format!("{mantissa}{EXPONENT_MARKER}{exponent}"),
    )
}

// `convertNumberToScientificNotation`.
fn to_scientific(number: &str, as_float: bool) -> Scientific {
    let (coefficient, exponent) = number
        .split_once(EXPONENT_MARKER)
        .map_or((number, None), |(coefficient, exponent)| {
            (coefficient, Some(exponent))
        });
    let mut normalized = if as_float || number.contains(DECIMAL_POINT) {
        normalize_float(coefficient)
    } else {
        normalize_integer(coefficient)
    };
    let shift = exponent
        .and_then(|exponent| exponent.parse::<i64>().ok())
        .unwrap_or_default();
    normalized.magnitude = normalized.magnitude.saturating_add(shift);
    normalized
}

fn normalize_integer(integer: &str) -> Scientific {
    let trimmed = integer.trim_start_matches(ZERO);
    let trimmed = if trimmed.is_empty() { integer } else { trimmed };
    let significant = trimmed.trim_end_matches(ZERO);
    let significant = if significant.is_empty() {
        trimmed
    } else {
        significant
    };
    Scientific {
        coefficient: significant.to_owned(),
        magnitude: length(trimmed).saturating_sub(1),
    }
}

fn normalize_float(float: &str) -> Scientific {
    let trimmed = float.trim_start_matches(ZERO);
    let trimmed = if trimmed.is_empty() { float } else { trimmed };
    match trimmed.find(DECIMAL_POINT) {
        Some(0) => {
            let fraction = trimmed.get(1..).unwrap_or_default();
            let significant = fraction.trim_start_matches(ZERO);
            let significant = if significant.is_empty() {
                fraction
            } else {
                significant
            };
            Scientific {
                coefficient: significant.to_owned(),
                magnitude: length(significant).saturating_sub(length(trimmed)),
            }
        }
        Some(point) => Scientific {
            coefficient: trimmed.replacen(DECIMAL_POINT, "", 1),
            magnitude: i64::try_from(point).unwrap_or_default().saturating_sub(1),
        },
        None => Scientific {
            coefficient: trimmed.to_owned(),
            magnitude: length(trimmed).saturating_sub(1),
        },
    }
}

fn length(text: &str) -> i64 {
    i64::try_from(text.len()).unwrap_or(i64::MAX)
}
