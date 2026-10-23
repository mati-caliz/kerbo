use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

const ZERO: char = '0';

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MemberKind {
    Constant,
    Class,
    Other,
}

// `IsortSortKey` de ruff: primero las constantes, después las clases y al final el resto, cada
// grupo en orden natural.
pub fn isort_compare(first: &str, second: &str) -> Ordering {
    member_kind(first)
        .cmp(&member_kind(second))
        .then_with(|| natural_compare(first, second))
}

fn member_kind(value: &str) -> MemberKind {
    if value.chars().nth(1).is_some() && is_cased_uppercase(value) {
        MemberKind::Constant
    } else if value.starts_with(char::is_uppercase) {
        MemberKind::Class
    } else {
        MemberKind::Other
    }
}

fn is_cased_uppercase(value: &str) -> bool {
    !value.chars().any(char::is_lowercase) && value.chars().any(char::is_uppercase)
}

// `natord::compare`: los dígitos se comparan como números y los espacios no cuentan.
pub fn natural_compare(first: &str, second: &str) -> Ordering {
    let mut left = first.chars().peekable();
    let mut right = second.chars().peekable();
    loop {
        skip_whitespace(&mut left);
        skip_whitespace(&mut right);
        let (Some(left_char), Some(right_char)) = (left.peek().copied(), right.peek().copied()) else {
            return left.peek().is_some().cmp(&right.peek().is_some());
        };
        if left_char.is_ascii_digit() && right_char.is_ascii_digit() {
            let numbers = if left_char == ZERO || right_char == ZERO {
                compare_fraction(&mut left, &mut right)
            } else {
                compare_integer(&mut left, &mut right)
            };
            if numbers != Ordering::Equal {
                return numbers;
            }
            continue;
        }
        match left_char.cmp(&right_char) {
            Ordering::Equal => {
                left.next();
                right.next();
            }
            different @ (Ordering::Less | Ordering::Greater) => return different,
        }
    }
}

fn skip_whitespace(characters: &mut Peekable<Chars<'_>>) {
    while characters
        .next_if(|character| character.is_whitespace())
        .is_some()
    {}
}

fn next_digits(
    left: &mut Peekable<Chars<'_>>,
    right: &mut Peekable<Chars<'_>>,
) -> (Option<char>, Option<char>) {
    (
        left.peek().copied().filter(char::is_ascii_digit),
        right.peek().copied().filter(char::is_ascii_digit),
    )
}

// Un número sin ceros adelante: gana el más largo, y si miden lo mismo, el primer dígito distinto.
fn compare_integer(left: &mut Peekable<Chars<'_>>, right: &mut Peekable<Chars<'_>>) -> Ordering {
    let mut bias = Ordering::Equal;
    loop {
        match next_digits(left, right) {
            (None, None) => return bias,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_digit), Some(right_digit)) => {
                if bias == Ordering::Equal {
                    bias = left_digit.cmp(&right_digit);
                }
                left.next();
                right.next();
            }
        }
    }
}

// Un número con ceros adelante se compara dígito a dígito, como una parte decimal.
fn compare_fraction(left: &mut Peekable<Chars<'_>>, right: &mut Peekable<Chars<'_>>) -> Ordering {
    loop {
        match next_digits(left, right) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_digit), Some(right_digit)) => {
                let ordering = left_digit.cmp(&right_digit);
                if ordering != Ordering::Equal {
                    return ordering;
                }
                left.next();
                right.next();
            }
        }
    }
}
