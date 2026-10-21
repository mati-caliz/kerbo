mod literal;

use std::collections::HashSet;

use tree_sitter::Node;

use super::Report;
use super::ast::{IDENTIFIER, named, text};
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
    let mut seen: HashSet<Key> = HashSet::new();
    let mut reports = Vec::new();
    for pair in named(dictionary).into_iter().filter(|item| item.kind() == PAIR) {
        let Some(node) = pair.child_by_field_name(KEY_FIELD) else {
            continue;
        };
        let Some(identity) = key(node, source) else {
            continue;
        };
        let is_name = matches!(identity, Key::Name(_));
        if seen.insert(identity) {
            continue;
        }
        let rule = if is_name && node.kind() == IDENTIFIER {
            PythonRule::F602
        } else {
            PythonRule::F601
        };
        reports.push(Report::at(rule, node, text(node, source)));
    }
    reports
}
