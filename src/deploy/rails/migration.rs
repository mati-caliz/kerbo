use std::collections::HashMap;

use tree_sitter::Node;

use super::arguments::raw;
use crate::syntax::{descendants, named_children};

const CLASS: &str = "class";
const METHOD: &str = "method";
const SINGLETON_METHOD: &str = "singleton_method";
const CALL: &str = "call";
const CLASS_EVAL: &str = "class_eval";
const IDENTIFIER: &str = "identifier";
const MIGRATION_MARKER: &str = "Migration";
const DISABLE_TRANSACTION: &str = "disable_ddl_transaction!";
const VERSION_OPEN: char = '[';
const VERSION_CLOSE: char = ']';
const VERSION_SEPARATOR: char = '.';
const FIRST_BIGINT_VERSION: (u32, u32) = (5, 1);
const FIRST_INDEXED_REFERENCES_VERSION: (u32, u32) = (5, 0);
const UNVERSIONED: (u32, u32) = (0, 0);

#[derive(Debug, Default, Clone, Copy)]
pub struct Compatibility {
    pub integer_ids: bool,
    pub indexed_references: bool,
}

pub fn migration_class<'tree>(root: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    descendants(root).into_iter().find(|node| {
        node.kind() == CLASS
            && node
                .child_by_field_name("superclass")
                .is_some_and(|superclass| raw(superclass, source).contains(MIGRATION_MARKER))
    })
}

pub fn compatibility(class: Node<'_>, source: &[u8]) -> Compatibility {
    let version = version(class, source);
    Compatibility {
        integer_ids: version < FIRST_BIGINT_VERSION,
        indexed_references: version >= FIRST_INDEXED_REFERENCES_VERSION,
    }
}

fn version(class: Node<'_>, source: &[u8]) -> (u32, u32) {
    let superclass = class
        .child_by_field_name("superclass")
        .map(|superclass| raw(superclass, source))
        .unwrap_or_default();
    let Some(version) = superclass
        .split_once(VERSION_OPEN)
        .and_then(|(_, rest)| rest.split_once(VERSION_CLOSE))
        .map(|(version, _)| version)
    else {
        return UNVERSIONED;
    };
    let mut parts = version
        .split(VERSION_SEPARATOR)
        .map(|part| part.trim().parse::<u32>().unwrap_or_default());
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    (major, minor)
}

pub fn disables_transaction(class: Node<'_>, source: &[u8]) -> bool {
    descendants(class)
        .into_iter()
        .any(|node| node.kind() == IDENTIFIER && raw(node, source) == DISABLE_TRANSACTION)
}

pub fn helpers<'tree>(root: Node<'tree>, class: Node<'tree>, source: &[u8]) -> HashMap<String, Node<'tree>> {
    let mut found = HashMap::new();
    let class_name = class.child_by_field_name("name").map(|name| raw(name, source));
    let reopened = descendants(root)
        .into_iter()
        .filter(|node| is_class_eval_of(*node, class_name.as_deref(), source))
        .filter_map(|call| call.child_by_field_name("block")?.child_by_field_name("body"));
    for body in class.child_by_field_name("body").into_iter().chain(reopened) {
        found.extend(methods(body, source));
    }
    found
}

fn is_class_eval_of(node: Node<'_>, class_name: Option<&str>, source: &[u8]) -> bool {
    node.kind() == CALL
        && node
            .child_by_field_name("method")
            .is_some_and(|method| raw(method, source) == CLASS_EVAL)
        && node
            .child_by_field_name("receiver")
            .is_some_and(|receiver| Some(raw(receiver, source).as_str()) == class_name)
}

fn methods<'tree>(body: Node<'tree>, source: &[u8]) -> Vec<(String, Node<'tree>)> {
    named_children(body)
        .into_iter()
        .filter(|node| [METHOD, SINGLETON_METHOD].contains(&node.kind()))
        .filter_map(|method| {
            let method_name = raw(method.child_by_field_name("name")?, source);
            Some((method_name, method.child_by_field_name("body")?))
        })
        .collect()
}
