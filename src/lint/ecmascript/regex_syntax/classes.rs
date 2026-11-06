use super::escapes::character_escape;
use super::{Kind, Parser};

const HEX_RADIX: u32 = 16;

pub fn class(parser: &mut Parser, parent: usize) -> Result<(), &'static str> {
    let node = parser.open(
        Kind::Class {
            negate: false,
            empty: false,
        },
        Some(parent),
    );
    parser.advance();
    let negate = parser.eat('^');
    let mut elements = 0_usize;
    let mut previous: Option<Option<char>> = None;
    let mut in_range = false;
    loop {
        let Some(character) = parser.peek() else {
            return Err("Unterminated character class");
        };
        if character == ']' {
            parser.advance();
            break;
        }
        if parser.mode.unicode_sets && character == '[' {
            class(parser, node)?;
            elements = elements.saturating_add(1);
            continue;
        }
        if parser.mode.unicode_sets && parser.eat_text("\\q{") {
            skip_class_string(parser)?;
            elements = elements.saturating_add(1);
            continue;
        }
        let value = class_atom(parser)?;
        elements = elements.saturating_add(1);
        if in_range {
            check_range(parser, previous.flatten(), value)?;
            in_range = false;
            previous = None;
            continue;
        }
        if parser.peek() == Some('-')
            && parser.peek_at(1).is_some_and(|next| next != ']')
            && !parser.mode.unicode_sets
        {
            parser.advance();
            in_range = true;
        }
        previous = Some(value);
    }
    parser.set_kind(
        node,
        Kind::Class {
            negate,
            empty: elements == 0,
        },
    );
    parser.close(node);
    Ok(())
}

fn skip_class_string(parser: &mut Parser) -> Result<(), &'static str> {
    while let Some(character) = parser.advance() {
        if character == '}' {
            return Ok(());
        }
    }
    Err("Invalid escape")
}

const fn check_range(parser: &Parser, from: Option<char>, to: Option<char>) -> Result<(), &'static str> {
    match (from, to) {
        (Some(from), Some(to)) if from > to => Err("Range out of order in character class"),
        (None, _) | (_, None) if parser.mode.is_strict() => Err("Invalid character class"),
        _ => Ok(()),
    }
}

// El carácter de un elemento, o nada si es una clase como `\d`.
fn class_atom(parser: &mut Parser) -> Result<Option<char>, &'static str> {
    let Some(character) = parser.advance() else {
        return Err("Unterminated character class");
    };
    if character != '\\' {
        return Ok(Some(character));
    }
    let start = parser.position;
    character_escape(parser, true)?;
    let raw: String = parser
        .characters
        .get(start..parser.position)
        .unwrap_or_default()
        .iter()
        .map(|(_, escaped)| *escaped)
        .collect();
    Ok(escaped_value(&raw))
}

fn escaped_value(raw: &str) -> Option<char> {
    let mut characters = raw.chars();
    let first = characters.next()?;
    let rest = characters.as_str();
    match first {
        'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'p' | 'P' => None,
        'x' | 'u' => {
            let digits = rest.trim_start_matches('{').trim_end_matches('}');
            u32::from_str_radix(digits, HEX_RADIX)
                .ok()
                .and_then(char::from_u32)
                .or(Some(first))
        }
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'v' => Some('\u{b}'),
        'f' => Some('\u{c}'),
        'b' => Some('\u{8}'),
        '0' => Some('\0'),
        other => Some(other),
    }
}
