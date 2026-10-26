use tree_sitter::Node;

use super::ast::{CONCATENATED_STRING, NONE, named, string_prefix, text};
use super::escapes::text_value;
use super::fix_edits::commented_unsafe;
use crate::fix::Fix;

const UNARY: &str = "unary_operator";
const ARGUMENT_FIELD: &str = "argument";
const DIGIT_SEPARATOR: char = '_';
const RADIX_PREFIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];
const DECIMAL_RADIX: u32 = 10;
const RAW: [char; 2] = ['r', 'R'];
const UNSPLIT: i64 = -1;
const ELEMENT_SEPARATOR: &str = ", ";

// Un `split` o `rsplit` sobre un string fijo: la llamada, el string y lo que se le pasó.
#[derive(Clone, Copy)]
pub struct StaticSplit<'tree> {
    pub call: Node<'tree>,
    pub object: Node<'tree>,
    pub separator: Option<Node<'tree>>,
    pub maxsplit: Option<Node<'tree>>,
    pub reverse: bool,
}

// El arreglo de SIM905, como ruff: la lista con los pedazos del string, cada uno con el prefijo y la
// comilla del primer string. Sin arreglo cuando habría que escapar algo (ruff lo escapa).
pub fn split_list_fix(split: &StaticSplit<'_>, source: &[u8]) -> Option<Fix> {
    let StaticSplit {
        call,
        object,
        separator,
        maxsplit,
        reverse,
    } = *split;
    let value = text_value(object, source)?;
    let limit = maxsplit.map_or(Some(UNSPLIT), |maxsplit| integer_value(maxsplit, source))?;
    let pieces: Vec<String> = match separator.filter(|separator| separator.kind() != NONE) {
        Some(separator) => split_by(&value, &text_value(separator, source)?, limit, reverse),
        None => split_whitespace(&value, limit, reverse),
    };
    let first = if object.kind() == CONCATENATED_STRING {
        *named(object).first()?
    } else {
        object
    };
    let prefix = string_prefix(first, source);
    let quote = text(first, source).get(prefix.len()..)?.chars().next()?;
    if prefix.contains(RAW) || pieces.iter().any(|piece| needs_escaping(piece, quote)) {
        return None;
    }
    let listed: Vec<String> = pieces
        .iter()
        .map(|piece| format!("{prefix}{quote}{piece}{quote}"))
        .collect();
    let fix = Fix::replace(call, &format!("[{}]", listed.join(ELEMENT_SEPARATOR)), source);
    Some(commented_unsafe(fix, call, source))
}

fn needs_escaping(piece: &str, quote: char) -> bool {
    piece
        .chars()
        .any(|character| character == quote || character == '\\' || character.is_control())
}

fn integer_value(node: Node<'_>, source: &[u8]) -> Option<i64> {
    if node.kind() == UNARY {
        return integer_value(node.child_by_field_name(ARGUMENT_FIELD)?, source).map(|value| -value);
    }
    let digits = text(node, source)
        .replace(DIGIT_SEPARATOR, "")
        .to_ascii_lowercase();
    let (radix, digits) = RADIX_PREFIXES
        .iter()
        .find_map(|(prefix, radix)| digits.strip_prefix(prefix).map(|rest| (*radix, rest.to_owned())))
        .unwrap_or_else(|| (DECIMAL_RADIX, digits.clone()));
    i64::from_str_radix(&digits, radix).ok()
}

fn split_by(value: &str, separator: &str, limit: i64, reverse: bool) -> Vec<String> {
    let pieces: Vec<&str> = match (usize::try_from(limit), reverse) {
        (Ok(splits), false) => value.splitn(splits.saturating_add(1), separator).collect(),
        (Ok(splits), true) => value.rsplitn(splits.saturating_add(1), separator).collect(),
        (Err(_), false) => value.split(separator).collect(),
        (Err(_), true) => value.rsplit(separator).collect(),
    };
    let mut pieces: Vec<String> = pieces.into_iter().map(str::to_owned).collect();
    if reverse {
        pieces.reverse();
    }
    pieces
}

// `str.split()` sin separador: corta en los espacios de Python, sin pedazos vacíos; con un límite,
// el resto queda entero (sin los espacios del lado por donde se empezó).
fn split_whitespace(value: &str, limit: i64, reverse: bool) -> Vec<String> {
    let Ok(splits) = usize::try_from(limit) else {
        return value
            .split(is_python_whitespace)
            .filter(|piece| !piece.is_empty())
            .map(str::to_owned)
            .collect();
    };
    let mut pieces = Vec::new();
    let mut rest = if reverse {
        value.trim_end_matches(is_python_whitespace)
    } else {
        value.trim_start_matches(is_python_whitespace)
    };
    for _ in 0..splits {
        let boundary = if reverse {
            rest.rfind(is_python_whitespace)
        } else {
            rest.find(is_python_whitespace)
        };
        let Some(boundary) = boundary else {
            break;
        };
        if reverse {
            let width = rest
                .get(boundary..)
                .and_then(|tail| tail.chars().next())
                .map_or(1, char::len_utf8);
            pieces.push(
                rest.get(boundary.saturating_add(width)..)
                    .unwrap_or_default()
                    .to_owned(),
            );
            rest = rest
                .get(..boundary)
                .unwrap_or_default()
                .trim_end_matches(is_python_whitespace);
        } else {
            pieces.push(rest.get(..boundary).unwrap_or_default().to_owned());
            rest = rest
                .get(boundary..)
                .unwrap_or_default()
                .trim_start_matches(is_python_whitespace);
        }
    }
    if !rest.is_empty() {
        pieces.push(rest.to_owned());
    }
    if reverse {
        pieces.reverse();
    }
    pieces
}

fn is_python_whitespace(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}
