use super::values::Literal;

pub fn as_number(literal: &Literal) -> Option<i64> {
    match literal {
        Literal::Integer(number) => Some(*number),
        Literal::Character(character) => Some(i64::from(u32::from(*character))),
        Literal::Text(_) | Literal::Boolean(_) => None,
    }
}

pub fn as_text(literal: &Literal) -> String {
    match literal {
        Literal::Text(written) => written.clone(),
        Literal::Integer(number) => number.to_string(),
        Literal::Character(character) => character.to_string(),
        Literal::Boolean(flag) => flag.to_string(),
    }
}

pub fn fold_binary(operator: &str, left: &Literal, right: &Literal) -> Option<Literal> {
    if operator == "+" && (matches!(left, Literal::Text(_)) || matches!(right, Literal::Text(_))) {
        return Some(Literal::Text(as_text(left) + &as_text(right)));
    }
    if let (Literal::Boolean(one), Literal::Boolean(two)) = (left, right) {
        return match operator {
            "&&" => Some(Literal::Boolean(*one && *two)),
            "||" => Some(Literal::Boolean(*one || *two)),
            "==" => Some(Literal::Boolean(one == two)),
            "!=" => Some(Literal::Boolean(one != two)),
            _ => None,
        };
    }
    let (one, two) = (as_number(left)?, as_number(right)?);
    let number = match operator {
        "+" => one.checked_add(two)?,
        "-" => one.checked_sub(two)?,
        "*" => one.checked_mul(two)?,
        "/" => one.checked_div(two)?,
        "%" => one.checked_rem(two)?,
        "<" => return Some(Literal::Boolean(one < two)),
        ">" => return Some(Literal::Boolean(one > two)),
        "<=" => return Some(Literal::Boolean(one <= two)),
        ">=" => return Some(Literal::Boolean(one >= two)),
        "==" => return Some(Literal::Boolean(one == two)),
        "!=" => return Some(Literal::Boolean(one != two)),
        _ => return None,
    };
    Some(Literal::Integer(number))
}

pub fn fold_unary(operator: &str, operand: &Literal) -> Option<Literal> {
    match (operator, operand) {
        ("!", Literal::Boolean(flag)) => Some(Literal::Boolean(!flag)),
        ("-", Literal::Integer(number)) => number.checked_neg().map(Literal::Integer),
        _ => None,
    }
}

fn index_of(arguments: &[Literal], position: usize) -> Option<usize> {
    as_number(arguments.get(position)?).and_then(|number| usize::try_from(number).ok())
}

// Lo que devuelve un método de `String` sobre un texto constante con argumentos constantes.
pub fn fold_text_method(receiver: &str, method: &str, arguments: &[Literal]) -> Option<Literal> {
    let characters: Vec<char> = receiver.chars().collect();
    let first_text = || match arguments.first() {
        Some(Literal::Text(written)) => Some(written.as_str()),
        _ => None,
    };
    match method {
        "charAt" => characters
            .get(index_of(arguments, 0)?)
            .copied()
            .map(Literal::Character),
        "length" => i64::try_from(characters.len()).ok().map(Literal::Integer),
        "substring" => {
            let start = index_of(arguments, 0)?;
            let end = if arguments.len() > 1 {
                index_of(arguments, 1)?
            } else {
                characters.len()
            };
            characters
                .get(start..end)
                .map(|slice| Literal::Text(slice.iter().collect()))
        }
        "toUpperCase" => Some(Literal::Text(receiver.to_uppercase())),
        "toLowerCase" => Some(Literal::Text(receiver.to_lowercase())),
        "trim" => Some(Literal::Text(receiver.trim().to_owned())),
        "isEmpty" => Some(Literal::Boolean(receiver.is_empty())),
        "equals" => Some(Literal::Boolean(first_text()? == receiver)),
        "contains" => Some(Literal::Boolean(receiver.contains(first_text()?))),
        "startsWith" => Some(Literal::Boolean(receiver.starts_with(first_text()?))),
        "endsWith" => Some(Literal::Boolean(receiver.ends_with(first_text()?))),
        "concat" => Some(Literal::Text(format!("{receiver}{}", first_text()?))),
        _ => None,
    }
}

pub const fn truth(literal: Option<&Literal>) -> Option<bool> {
    match literal {
        Some(Literal::Boolean(flag)) => Some(*flag),
        _ => None,
    }
}

pub fn same_label(subject: &Literal, label: &Literal) -> bool {
    match (subject, label) {
        (Literal::Text(one), Literal::Text(two)) => one == two,
        _ => as_number(subject).is_some_and(|one| as_number(label) == Some(one)),
    }
}
