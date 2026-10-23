use std::collections::HashMap;

use tree_sitter::Node;

use super::spec_names;
use crate::syntax::{descendants, named_children};

const IDENTIFIER: &str = "identifier";
const INTERPRETED: &str = "interpreted_string_literal";
const RAW: &str = "raw_string_literal";
const BINARY: &str = "binary_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const CONST_SPEC: &str = "const_spec";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const PLUS: &str = "+";
const QUOTE: char = '"';
const BACKTICK: char = '`';
const CARRIAGE_RETURN: char = '\r';
// Las constantes se siguen hasta esta profundidad: alcanza y corta un ciclo.
const MAX_CONSTANT_DEPTH: usize = 8;
const HEX_DIGITS: [usize; 3] = [2, 4, 8];
const OCTAL_RADIX: u32 = 8;
const HEX_RADIX: u32 = 16;
const OCTAL_TAIL: usize = 2;

// Las constantes de texto del archivo por nombre; un nombre declarado dos veces no se usa.
pub fn constant_specs<'tree>(root: Node<'tree>, source: &[u8]) -> HashMap<String, Option<Node<'tree>>> {
    let mut constants: HashMap<String, Option<Node<'tree>>> = HashMap::new();
    for spec in descendants(root)
        .into_iter()
        .filter(|node| node.kind() == CONST_SPEC)
    {
        let values: Vec<Node<'_>> = spec
            .child_by_field_name(VALUE_FIELD)
            .map(named_children)
            .unwrap_or_default();
        for (position, name) in spec_names(spec).iter().enumerate() {
            let written = name.utf8_text(source).unwrap_or_default().to_owned();
            let value = values.get(position).copied();
            constants
                .entry(written)
                .and_modify(|existing| *existing = None)
                .or_insert(value);
        }
    }
    constants
}

// El texto constante de una expresión: literales, sumas de textos y constantes del archivo.
pub fn constant_text(
    node: Node<'_>,
    source: &[u8],
    constants: &HashMap<String, Option<Node<'_>>>,
    depth: usize,
) -> Option<String> {
    if depth > MAX_CONSTANT_DEPTH {
        return None;
    }
    let deeper = depth.saturating_add(1);
    let written = node.utf8_text(source).ok()?;
    match node.kind() {
        INTERPRETED => unescape(written.strip_prefix(QUOTE)?.strip_suffix(QUOTE)?),
        RAW => Some(
            written
                .strip_prefix(BACKTICK)?
                .strip_suffix(BACKTICK)?
                .replace(CARRIAGE_RETURN, ""),
        ),
        PARENTHESIZED => constant_text(
            named_children(node).into_iter().next()?,
            source,
            constants,
            deeper,
        ),
        BINARY => {
            if node.child_by_field_name(OPERATOR_FIELD)?.utf8_text(source).ok()? != PLUS {
                return None;
            }
            let left = constant_text(node.child_by_field_name(LEFT_FIELD)?, source, constants, deeper)?;
            let right = constant_text(node.child_by_field_name(RIGHT_FIELD)?, source, constants, deeper)?;
            Some(left + &right)
        }
        IDENTIFIER => constant_text((*constants.get(written)?)?, source, constants, deeper),
        _ => None,
    }
}

// El valor de un literal interpretado de Go; con un escape que no se reconoce, nada.
fn unescape(body: &str) -> Option<String> {
    let mut decoded = String::with_capacity(body.len());
    let mut letters = body.chars();
    while let Some(letter) = letters.next() {
        if letter != '\\' {
            decoded.push(letter);
            continue;
        }
        let [byte_digits, short_digits, long_digits] = HEX_DIGITS;
        let escaped = match letters.next()? {
            'a' => '\u{7}',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\u{b}',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            'x' => numeric_escape(&mut letters, String::new(), byte_digits, HEX_RADIX)?,
            'u' => numeric_escape(&mut letters, String::new(), short_digits, HEX_RADIX)?,
            'U' => numeric_escape(&mut letters, String::new(), long_digits, HEX_RADIX)?,
            digit @ '0'..='7' => numeric_escape(&mut letters, digit.to_string(), OCTAL_TAIL, OCTAL_RADIX)?,
            _ => return None,
        };
        decoded.push(escaped);
    }
    Some(decoded)
}

fn numeric_escape(
    letters: &mut impl Iterator<Item = char>,
    mut written: String,
    digits: usize,
    radix: u32,
) -> Option<char> {
    written.extend(letters.take(digits));
    char::from_u32(u32::from_str_radix(&written, radix).ok()?)
}
