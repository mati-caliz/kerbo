use tree_sitter::Node;

use super::constants::Constants;
use super::nodes::{ARGUMENTS, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, STRING, without_parentheses};
use super::strings::static_string;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const REGEX: &str = "regex";
const PATTERN_FIELD: &str = "pattern";
const FLAGS_FIELD: &str = "flags";
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const REG_EXP: &str = "RegExp";
const UNICODE_FLAGS: &[char] = &['u', 'v'];
const LAST_CONTROL: u32 = 0x1f;
const BACKSLASH: char = '\\';
const HEX_RADIX: u32 = 16;
const HEX_ESCAPE_DIGITS: usize = 2;
const UNICODE_ESCAPE_DIGITS: usize = 4;

// Un patrón de expresión regular con sus flags: un literal `/…/` o el primer argumento de `RegExp`.
struct Pattern<'tree> {
    node: Node<'tree>,
    source: String,
    unicode: bool,
}

// `no-control-regex`: un carácter de control (de U+0000 a U+001F) dentro de una expresión regular.
pub fn control_characters_in_regexes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    let literals = context
        .of_kind(&[REGEX])
        .map(|regex| literal_pattern(regex, context));
    let constructed = context
        .of_kind(&[STRING])
        .filter_map(|string| constructed_pattern(string, &constants, context));
    literals
        .chain(constructed)
        .filter(|pattern| has_control_character(&pattern.source, pattern.unicode))
        .map(|pattern| report(EslintRule::NoControlRegex, pattern.node, ""))
        .collect()
}

fn literal_pattern<'tree>(regex: Node<'tree>, context: &Context<'tree, '_>) -> Pattern<'tree> {
    let source = regex
        .child_by_field_name(PATTERN_FIELD)
        .map_or("", |pattern| context.text(pattern));
    let flags = regex
        .child_by_field_name(FLAGS_FIELD)
        .map_or("", |flags| context.text(flags));
    Pattern {
        node: regex,
        source: source.to_owned(),
        unicode: flags.contains(UNICODE_FLAGS),
    }
}

fn constructed_pattern<'tree>(
    string: Node<'tree>,
    constants: &Constants<'_, 'tree, '_>,
    context: &Context<'tree, '_>,
) -> Option<Pattern<'tree>> {
    let arguments = string.parent().filter(|parent| parent.kind() == ARGUMENTS)?;
    let values: Vec<Node<'tree>> = {
        let mut cursor = arguments.walk();
        arguments
            .named_children(&mut cursor)
            .filter(|child| child.kind() != COMMENT)
            .collect()
    };
    if values.first().is_none_or(|first| first.id() != string.id()) {
        return None;
    }
    let call = arguments.parent()?;
    let callee = match call.kind() {
        CALL => call.child_by_field_name(FUNCTION_FIELD),
        NEW => call.child_by_field_name(CONSTRUCTOR_FIELD),
        _ => None,
    }?;
    let is_reg_exp =
        callee.kind() == IDENTIFIER && context.text(callee) == REG_EXP && constants.is_global(callee);
    if !is_reg_exp {
        return None;
    }
    let flags = values
        .get(1)
        .map(|flags| without_parentheses(*flags))
        .filter(|flags| flags.kind() == STRING)
        .and_then(|flags| static_string(flags, context.source))
        .unwrap_or_default();
    Some(Pattern {
        node: string,
        source: static_string(string, context.source)?,
        unicode: flags.contains(UNICODE_FLAGS),
    })
}

// Los caracteres que regexpp entrega como `onCharacter` con un código de control: escritos tal cual
// o con `\x` y `\u` (no `\cX` ni `\t` y compañía, que ESLint deja pasar).
fn has_control_character(pattern: &str, unicode: bool) -> bool {
    let mut characters = pattern.chars();
    while let Some(character) = characters.next() {
        if character != BACKSLASH {
            if u32::from(character) <= LAST_CONTROL {
                return true;
            }
            continue;
        }
        let escaped = characters.next();
        let rest: String = characters.clone().collect();
        let code = match escaped {
            Some('x') => hex_code(&rest, HEX_ESCAPE_DIGITS),
            Some('u') => unicode_code(&rest, unicode),
            _ => None,
        };
        if code.is_some_and(|code| code <= LAST_CONTROL) {
            return true;
        }
    }
    false
}

fn hex_code(rest: &str, digits: usize) -> Option<u32> {
    parse_hex(rest.get(..digits)?)
}

fn parse_hex(digits: &str) -> Option<u32> {
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, HEX_RADIX).ok()
}

fn unicode_code(rest: &str, unicode: bool) -> Option<u32> {
    if unicode && let Some(braced) = rest.strip_prefix('{') {
        let end = braced.find('}')?;
        return parse_hex(braced.get(..end)?);
    }
    hex_code(rest, UNICODE_ESCAPE_DIGITS)
}
