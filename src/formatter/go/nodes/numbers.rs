use super::super::token::Token;

const PREFIX_LENGTH: usize = 2;
const IMAGINARY_SUFFIX: char = 'i';
const IMAGINARY_ZERO: &str = "0i";

// Port de `normalizedNumber` de go/printer: prefijos y exponentes en minúscula y sin ceros a la
// izquierda en los imaginarios enteros. Los dígitos hexadecimales no se tocan.
pub fn normalized(kind: Token, value: &str) -> String {
    if !matches!(kind, Token::Int | Token::Float | Token::Imag) || value.len() < PREFIX_LENGTH {
        return value.to_owned();
    }
    let prefix = value.get(..PREFIX_LENGTH).unwrap_or_default();
    let digits = value.get(PREFIX_LENGTH..).unwrap_or_default();
    match prefix {
        "0X" => replace_last(&format!("0x{digits}"), 'P', 'p'),
        "0x" => replace_last(value, 'P', 'p'),
        "0O" => format!("0o{digits}"),
        "0B" => format!("0b{digits}"),
        "0o" | "0b" => value.to_owned(),
        _ => decimal(value),
    }
}

fn decimal(value: &str) -> String {
    if value.contains('E') {
        return replace_last(value, 'E', 'e');
    }
    if value.ends_with(IMAGINARY_SUFFIX) && !value.contains(['.', 'e']) {
        let trimmed = value.trim_start_matches(['0', '_']);
        if trimmed == "i" {
            return IMAGINARY_ZERO.to_owned();
        }
        return trimmed.to_owned();
    }
    value.to_owned()
}

fn replace_last(value: &str, from: char, to: char) -> String {
    value.rfind(from).map_or_else(
        || value.to_owned(),
        |index| {
            let mut replaced = value.to_owned();
            replaced.replace_range(index..=index, &to.to_string());
            replaced
        },
    )
}
