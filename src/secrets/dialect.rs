const ESCAPE: char = '\\';
const CLASS_OPEN: char = '[';
const CLASS_CLOSE: char = ']';
const CLASS_NEGATION: char = '^';
const NAMED_CLASS_OPEN: &str = "[:";
const NAMED_CLASS_CLOSE: &str = ":]";
const REPEAT_OPEN: char = '{';
const REPEAT_CLOSE: char = '}';
const REPEAT_SEPARATOR: char = ',';

pub fn to_rust(pattern: &str) -> String {
    let mut translated = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(character) = rest.chars().next() {
        let consumed = match character {
            ESCAPE => escaped_length(rest),
            CLASS_OPEN => class_length(rest),
            REPEAT_OPEN => repetition_length(rest).unwrap_or(0),
            _ => character.len_utf8(),
        };
        if consumed == 0 {
            translated.push(ESCAPE);
            translated.push(character);
            rest = rest.get(character.len_utf8()..).unwrap_or_default();
            continue;
        }
        let (taken, remaining) = rest.split_at(consumed.min(rest.len()));
        if character == REPEAT_CLOSE {
            translated.push(ESCAPE);
        }
        translated.push_str(taken);
        rest = remaining;
    }
    translated
}

fn escaped_length(text: &str) -> usize {
    let next = text.chars().nth(1).map_or(0, char::len_utf8);
    ESCAPE.len_utf8().saturating_add(next)
}

fn class_length(text: &str) -> usize {
    let mut offset = CLASS_OPEN.len_utf8();
    if text
        .get(offset..)
        .is_some_and(|rest| rest.starts_with(CLASS_NEGATION))
    {
        offset = offset.saturating_add(CLASS_NEGATION.len_utf8());
    }
    if text
        .get(offset..)
        .is_some_and(|rest| rest.starts_with(CLASS_CLOSE))
    {
        offset = offset.saturating_add(CLASS_CLOSE.len_utf8());
    }
    while let Some(rest) = text.get(offset..) {
        let Some(character) = rest.chars().next() else {
            return text.len();
        };
        if rest.starts_with(NAMED_CLASS_OPEN)
            && let Some(close) = rest.find(NAMED_CLASS_CLOSE)
        {
            offset = offset
                .saturating_add(close)
                .saturating_add(NAMED_CLASS_CLOSE.len());
            continue;
        }
        offset = offset.saturating_add(match character {
            ESCAPE => escaped_length(rest),
            _ => character.len_utf8(),
        });
        if character == CLASS_CLOSE {
            return offset;
        }
    }
    text.len()
}

fn repetition_length(text: &str) -> Option<usize> {
    let body = text.get(REPEAT_OPEN.len_utf8()..)?;
    let close = body.find(REPEAT_CLOSE)?;
    let inside = body.get(..close)?;
    let (low, high) = inside.split_once(REPEAT_SEPARATOR).unwrap_or((inside, "0"));
    let digits = |part: &str| part.chars().all(|digit| digit.is_ascii_digit());
    let valid = !low.is_empty() && digits(low) && digits(high);
    valid.then(|| {
        close
            .saturating_add(REPEAT_OPEN.len_utf8())
            .saturating_add(REPEAT_CLOSE.len_utf8())
    })
}
