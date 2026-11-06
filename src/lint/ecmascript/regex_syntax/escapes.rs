use super::{Kind, Parser, Reference, RegexNode};

const SYNTAX_CHARACTERS: &str = "^$\\.*+?()[]{}|/";
const CLASS_ESCAPES: &str = "dDsSwW";
const MAX_CODE_POINT: u32 = 0x10_FFFF;
const HEX_RADIX: u32 = 16;
const HEX_ESCAPE_DIGITS: usize = 2;
const UNICODE_ESCAPE_DIGITS: usize = 4;

pub fn escape(parser: &mut Parser, parent: usize) -> Result<bool, &'static str> {
    let start = parser.byte_position();
    parser.advance();
    let Some(next) = parser.peek() else {
        return Err("\\ at end of pattern");
    };
    let reference = match next {
        'b' | 'B' => {
            parser.advance();
            return Ok(false);
        }
        '1'..='9' => numeric_escape(parser)?,
        'k' => named_reference(parser)?,
        _ => {
            character_escape(parser, false)?;
            None
        }
    };
    if let Some(reference) = reference {
        let end = parser.byte_position();
        parser.tree.nodes.push(RegexNode {
            kind: Kind::Backreference(reference),
            parent: Some(parent),
            start,
            end,
        });
    }
    Ok(true)
}

fn numeric_escape(parser: &mut Parser) -> Result<Option<Reference>, &'static str> {
    let saved = parser.position;
    let number = parser
        .number()
        .and_then(|number| usize::try_from(number).ok())
        .unwrap_or(usize::MAX);
    if number <= parser.capture_count {
        return Ok(Some(Reference::Number(number)));
    }
    if parser.mode.is_strict() {
        return Err("Invalid escape");
    }
    parser.position = saved.saturating_add(1);
    Ok(None)
}

// Sin grupos con nombre y sin `u`, `\k<n>` es texto, como en el anexo B de la especificación.
fn named_reference(parser: &mut Parser) -> Result<Option<Reference>, &'static str> {
    parser.advance();
    if !parser.mode.is_strict() && !parser.has_named_groups {
        return Ok(None);
    }
    if !parser.eat('<') {
        return Err("Invalid named reference");
    }
    parser.group_name().map(|name| Some(Reference::Name(name)))
}

pub fn character_escape(parser: &mut Parser, in_class: bool) -> Result<(), &'static str> {
    let strict = parser.mode.is_strict();
    let Some(next) = parser.advance() else {
        return Err("\\ at end of pattern");
    };
    match next {
        'c' => control_escape(parser, in_class),
        'x' => fixed_hex(parser, HEX_ESCAPE_DIGITS, strict, "Invalid escape"),
        'u' => unicode_escape(parser, strict),
        'p' | 'P' if strict => property_escape(parser),
        '0' if strict && parser.peek().is_some_and(|digit| digit.is_ascii_digit()) => {
            Err("Invalid decimal escape")
        }
        _ if !strict => Ok(()),
        _ if SYNTAX_CHARACTERS.contains(next)
            || CLASS_ESCAPES.contains(next)
            || matches!(next, 'f' | 'n' | 'r' | 't' | 'v' | '0')
            || (in_class && (next == '-' || next == 'b')) =>
        {
            Ok(())
        }
        _ if parser.mode.unicode_sets && in_class && "&-!#%,:;<=>@`~".contains(next) => Ok(()),
        _ => Err("Invalid escape"),
    }
}

fn control_escape(parser: &mut Parser, in_class: bool) -> Result<(), &'static str> {
    let strict = parser.mode.is_strict();
    let letter = parser.peek().is_some_and(|next| next.is_ascii_alphabetic());
    let class_digit = in_class
        && parser
            .peek()
            .is_some_and(|next| next.is_ascii_digit() || next == '_');
    if letter || (class_digit && !strict) {
        parser.advance();
        return Ok(());
    }
    if strict {
        Err("Invalid unicode escape")
    } else {
        Ok(())
    }
}

fn fixed_hex(
    parser: &mut Parser,
    digits: usize,
    strict: bool,
    message: &'static str,
) -> Result<(), &'static str> {
    let all_hex = (0..digits).all(|offset| {
        parser
            .peek_at(offset)
            .is_some_and(|digit| digit.is_digit(HEX_RADIX))
    });
    if all_hex {
        parser.position = parser.position.saturating_add(digits);
        return Ok(());
    }
    if strict { Err(message) } else { Ok(()) }
}

fn unicode_escape(parser: &mut Parser, strict: bool) -> Result<(), &'static str> {
    if strict && parser.eat('{') {
        let mut digits = String::new();
        while let Some(digit) = parser.peek().filter(|digit| digit.is_digit(HEX_RADIX)) {
            digits.push(digit);
            parser.advance();
        }
        let valid = u32::from_str_radix(&digits, HEX_RADIX).is_ok_and(|code| code <= MAX_CODE_POINT);
        return if valid && parser.eat('}') {
            Ok(())
        } else {
            Err("Invalid Unicode escape")
        };
    }
    fixed_hex(parser, UNICODE_ESCAPE_DIGITS, strict, "Invalid Unicode escape")
}

fn property_escape(parser: &mut Parser) -> Result<(), &'static str> {
    if !parser.eat('{') {
        return Err("Invalid property name");
    }
    while let Some(character) = parser.advance() {
        if character == '}' {
            return Ok(());
        }
    }
    Err("Invalid property name")
}
