use std::collections::HashMap;

use tree_sitter::Node;

use super::arguments::{name, quoted, raw};
use crate::syntax::named_children;

const PAIR: &str = "pair";
const HASH: &str = "hash";
const NIL: &str = "nil";
const QUOTED_KINDS: &[&str] = &["string", "simple_symbol", "delimited_symbol", "hash_key_symbol"];
const VERBATIM_KINDS: &[&str] = &["integer", "float", "true", "false"];
const SQL_NULL: &str = "NULL";
const NAMED_MARKER: char = ':';
const POSITIONAL_MARKER: char = '?';
const ESCAPE: char = '\\';
const DIGIT_SEPARATOR: char = '_';

pub fn bound(sql: &str, parameters: &[Node<'_>], source: &[u8]) -> Option<String> {
    match parameters {
        [] => Some(sql.to_owned()),
        [hash] if hash.kind() == HASH => named(sql, &named_children(*hash), source),
        pairs if pairs.iter().all(|parameter| parameter.kind() == PAIR) => named(sql, pairs, source),
        values => positional(sql, values, source),
    }
}

fn literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        NIL => Some(SQL_NULL.to_owned()),
        kind if VERBATIM_KINDS.contains(&kind) => {
            Some(raw(node, source).replace(DIGIT_SEPARATOR, "").to_uppercase())
        }
        kind if QUOTED_KINDS.contains(&kind) => name(node, source).map(|text| quoted(&text)),
        _ => None,
    }
}

fn named(sql: &str, pairs: &[Node<'_>], source: &[u8]) -> Option<String> {
    let values = pairs
        .iter()
        .map(|pair| {
            let key = pair.child_by_field_name("key")?;
            let key = name(key, source).unwrap_or_else(|| raw(key, source));
            Some((key, literal(pair.child_by_field_name("value")?, source)?))
        })
        .collect::<Option<HashMap<_, _>>>()?;
    let characters: Vec<char> = sql.chars().collect();
    let mut result = String::new();
    let mut index = 0;
    while let Some(&character) = characters.get(index) {
        let starts_name = characters.get(index + 1).is_some_and(char::is_ascii_alphabetic);
        if character != NAMED_MARKER || !starts_name {
            result.push(character);
            index += 1;
            continue;
        }
        let end = characters
            .iter()
            .skip(index + 1)
            .position(|next| !(next.is_ascii_alphanumeric() || *next == '_'))
            .map_or(characters.len(), |offset| index + 1 + offset);
        let identifier: String = characters.get(index + 1..end)?.iter().collect();
        match result.chars().last() {
            Some(NAMED_MARKER) => {
                result.push(character);
                result.push_str(&identifier);
            }
            Some(ESCAPE) => {
                result.pop();
                result.push(character);
                result.push_str(&identifier);
            }
            _ => result.push_str(values.get(&identifier)?),
        }
        index = end;
    }
    Some(result)
}

fn positional(sql: &str, values: &[Node<'_>], source: &[u8]) -> Option<String> {
    let mut literals = values
        .iter()
        .map(|value| literal(*value, source))
        .collect::<Option<Vec<_>>>()?
        .into_iter();
    let mut result = String::new();
    for character in sql.chars() {
        if character != POSITIONAL_MARKER {
            result.push(character);
        } else if result.ends_with(ESCAPE) {
            result.pop();
            result.push(character);
        } else {
            result.push_str(&literals.next()?);
        }
    }
    literals.next().is_none().then_some(result)
}
