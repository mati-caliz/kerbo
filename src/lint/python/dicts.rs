mod literal;

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, IDENTIFIER, VALUE_FIELD, named, text};
use super::branches::equivalent;
use super::fix_edits::delete_between;
use crate::fix::Fix;
use crate::rules::PythonRule;
use literal::{Key, key};

const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == DICTIONARY)
        .flat_map(|dictionary| repeated_keys(*dictionary, source))
        .collect()
}

fn repeated_keys(dictionary: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut seen: HashMap<Key, Vec<Node<'_>>> = HashMap::new();
    let mut reports = Vec::new();
    let items: Vec<Node<'_>> = named(dictionary)
        .into_iter()
        .filter(|item| item.kind() != COMMENT)
        .collect();
    for (position, pair) in items.iter().enumerate().filter(|(_, item)| item.kind() == PAIR) {
        let (Some(node), Some(value)) = (
            pair.child_by_field_name(KEY_FIELD),
            pair.child_by_field_name(VALUE_FIELD),
        ) else {
            continue;
        };
        let Some(identity) = key(node, source) else {
            continue;
        };
        let is_name = matches!(identity, Key::Name(_));
        let values = match seen.entry(identity) {
            Entry::Vacant(vacant) => {
                vacant.insert(vec![value]);
                continue;
            }
            Entry::Occupied(occupied) => occupied.into_mut(),
        };
        let repeats_value = values
            .iter()
            .any(|seen_value| equivalent(*seen_value, value, source));
        if !repeats_value {
            values.push(value);
        }
        let rule = if is_name && node.kind() == IDENTIFIER {
            PythonRule::F602
        } else {
            PythonRule::F601
        };
        let fix = position
            .checked_sub(1)
            .and_then(|previous| items.get(previous))
            .filter(|_| repeats_value)
            .map(|previous| repeated_item_removal(*previous, *pair, source));
        reports.push(Report::at(rule, node, text(node, source)).with_fix(fix));
    }
    reports
}

// Como ruff: desde el final del elemento anterior hasta el final del repetido, así se va con su coma.
fn repeated_item_removal(previous: Node<'_>, repeated: Node<'_>, source: &[u8]) -> Fix {
    delete_between(previous.end_byte(), repeated.end_byte(), source).marked_unsafe()
}
