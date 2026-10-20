use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::{named_children, one_based};

const MERGE_KEY: &str = "<<";
const MAX_ALIAS_DEPTH: usize = 16;
const QUOTES: &[char] = &['"', '\''];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub line: usize,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Map(Vec<Entry>),
    Sequence(Vec<Self>),
    Scalar(String),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.entry(key).map(|entry| &entry.value)
    }

    pub fn entry(&self, key: &str) -> Option<&Entry> {
        match self {
            Self::Map(entries) => entries.iter().find(|entry| entry.key == key),
            Self::Sequence(_) | Self::Scalar(_) => None,
        }
    }

    pub fn path(&self, keys: &[&str]) -> Option<&Self> {
        keys.iter().try_fold(self, |value, key| value.get(key))
    }

    pub fn scalar(&self) -> Option<&str> {
        match self {
            Self::Scalar(text) => Some(text),
            Self::Map(_) | Self::Sequence(_) => None,
        }
    }

    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Map(entries) => entries,
            Self::Sequence(_) | Self::Scalar(_) => &[],
        }
    }
}

struct Converter<'source> {
    source: &'source [u8],
    anchors: HashMap<String, Node<'source>>,
}

pub fn document(root: Node<'_>, source: &[u8]) -> Option<Value> {
    let mut converter = Converter {
        source,
        anchors: HashMap::new(),
    };
    let document = named_children(root)
        .into_iter()
        .find(|child| child.kind() == "document")?;
    let content = named_children(document)
        .into_iter()
        .find(|child| child.kind() == "block_node" || child.kind() == "flow_node")?;
    converter.collect_anchors(content);
    Some(converter.value(content, 0))
}

impl<'source> Converter<'source> {
    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.source).unwrap_or_default().to_owned()
    }

    fn collect_anchors(&mut self, node: Node<'source>) {
        let mut pending = vec![node];
        while let Some(current) = pending.pop() {
            let children = named_children(current);
            let anchors: Vec<String> = children
                .iter()
                .filter(|child| child.kind() == "anchor")
                .filter_map(|anchor| self.part_text(*anchor, "anchor_name"))
                .collect();
            for name in anchors {
                self.anchors.insert(name, current);
            }
            pending.extend(children);
        }
    }

    fn part_text(&self, node: Node<'_>, kind: &str) -> Option<String> {
        named_children(node)
            .into_iter()
            .find(|part| part.kind() == kind)
            .map(|part| self.text(part))
    }

    fn value(&self, node: Node<'source>, depth: usize) -> Value {
        let content = named_children(node)
            .into_iter()
            .rfind(|child| child.kind() != "anchor" && child.kind() != "tag" && child.kind() != "comment");
        let Some(content) = content else {
            return Value::Scalar(String::new());
        };
        match content.kind() {
            "block_mapping" | "flow_mapping" => Value::Map(self.entries(content, depth)),
            "block_sequence" | "flow_sequence" => Value::Sequence(
                named_children(content)
                    .into_iter()
                    .filter(|item| item.kind() != "comment")
                    .map(|item| self.item(item, depth))
                    .collect(),
            ),
            "alias" => self.alias(content, depth),
            "block_node" | "flow_node" => self.value(content, depth),
            _ => Value::Scalar(self.text(content).trim().trim_matches(QUOTES).to_owned()),
        }
    }

    fn item(&self, item: Node<'source>, depth: usize) -> Value {
        if item.kind() == "block_sequence_item" {
            return named_children(item)
                .into_iter()
                .find(|child| child.kind() != "comment")
                .map_or_else(|| Value::Scalar(String::new()), |child| self.value(child, depth));
        }
        if item.kind() == "flow_pair" {
            return Value::Map(self.pair(item, depth).into_iter().collect());
        }
        self.value(item, depth)
    }

    fn alias(&self, alias: Node<'source>, depth: usize) -> Value {
        self.part_text(alias, "alias_name")
            .and_then(|name| self.anchors.get(&name))
            .filter(|_| depth < MAX_ALIAS_DEPTH)
            .map_or_else(
                || Value::Scalar(String::new()),
                |anchored| self.value(*anchored, depth.saturating_add(1)),
            )
    }

    fn pair(&self, pair: Node<'source>, depth: usize) -> Option<Entry> {
        let key = pair.child_by_field_name("key")?;
        let key_text = match self.value(key, depth) {
            Value::Scalar(text) => text,
            Value::Map(_) | Value::Sequence(_) => self.text(key),
        };
        let value = pair
            .child_by_field_name("value")
            .map_or_else(|| Value::Scalar(String::new()), |value| self.value(value, depth));
        Some(Entry {
            key: key_text,
            line: one_based(key.start_position().row),
            value,
        })
    }

    fn entries(&self, mapping: Node<'source>, depth: usize) -> Vec<Entry> {
        let mut explicit = Vec::new();
        let mut merged = Vec::new();
        for pair in named_children(mapping) {
            let Some(entry) = self.pair(pair, depth) else {
                continue;
            };
            if entry.key == MERGE_KEY {
                merged.extend(merged_entries(entry.value));
            } else {
                explicit.push(entry);
            }
        }
        for entry in merged {
            if !explicit.iter().any(|known: &Entry| known.key == entry.key) {
                explicit.push(entry);
            }
        }
        explicit
    }
}

fn merged_entries(value: Value) -> Vec<Entry> {
    match value {
        Value::Map(entries) => entries,
        Value::Sequence(values) => values.into_iter().flat_map(merged_entries).collect(),
        Value::Scalar(_) => Vec::new(),
    }
}
