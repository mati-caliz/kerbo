use tree_sitter::Node;

use super::condition::Condition;
use super::model::{Constraint, ConstraintKind, Index};
use super::values::{boolean, elements, is_sequence, strings, text};
use crate::deploy::python_calls::Call;

const UNIQUE_CONSTRAINT: &str = "UniqueConstraint";
const CHECK_CONSTRAINT: &str = "CheckConstraint";
const INDEX_ONLY_UNIQUE_KEYWORDS: &[&str] = &["condition", "include", "opclasses"];
const NULLS_DISTINCT: &str = "nulls_distinct";
const CHECK_KEYWORDS: &[&str] = &["condition", "check"];
const CONDITION: &str = "condition";
const EXPRESSION_PLACEHOLDER: &str = "(true)";
const OPERATOR_CLASSES: &str = "opclasses";
const INDEX_METHODS: &[(&str, &str)] = &[
    ("BTreeIndex", "btree"),
    ("BloomIndex", "bloom"),
    ("BrinIndex", "brin"),
    ("GinIndex", "gin"),
    ("GistIndex", "gist"),
    ("HashIndex", "hash"),
    ("SpGistIndex", "spgist"),
];

pub fn unique_together(node: Node<'_>, source: &[u8]) -> Vec<Vec<String>> {
    let sets = elements(node);
    if sets.iter().all(|set| !is_sequence(*set)) {
        let single = strings(node, source);
        return if single.is_empty() {
            Vec::new()
        } else {
            vec![single]
        };
    }
    sets.into_iter()
        .map(|set| strings(set, source))
        .filter(|set| !set.is_empty())
        .collect()
}

pub fn index(node: Node<'_>, source: &[u8]) -> Option<Index> {
    let call = Call::parse(node, source)?;
    let name = call.keyword("name").and_then(|value| text(value, source))?;
    let mut fields = call
        .keyword("fields")
        .map(|value| strings(value, source))
        .unwrap_or_default();
    if fields.is_empty() {
        fields = call
            .all_positional()
            .iter()
            .map(|_| EXPRESSION_PLACEHOLDER.to_owned())
            .collect();
    }
    Some(Index {
        name,
        fields,
        condition: parsed_condition(&call, source),
        method: INDEX_METHODS
            .iter()
            .find(|(class, _)| *class == call.method)
            .map(|(_, method)| *method),
        opclasses: operator_classes(&call, source),
    })
}

pub fn constraint(node: Node<'_>, source: &[u8]) -> Option<Constraint> {
    let call = Call::parse(node, source)?;
    let name = call.keyword("name").and_then(|value| text(value, source))?;
    let kind = match call.method {
        UNIQUE_CONSTRAINT => {
            let fields = call
                .keyword("fields")
                .map(|value| strings(value, source))
                .unwrap_or_default();
            let by_index = fields.is_empty()
                || INDEX_ONLY_UNIQUE_KEYWORDS
                    .iter()
                    .any(|keyword| call.keyword(keyword).is_some());
            ConstraintKind::Unique {
                fields,
                by_index,
                condition: parsed_condition(&call, source),
                opclasses: operator_classes(&call, source),
                nulls_distinct: call.keyword(NULLS_DISTINCT).and_then(boolean),
            }
        }
        CHECK_CONSTRAINT => ConstraintKind::Check {
            condition: CHECK_KEYWORDS
                .iter()
                .find_map(|keyword| call.keyword(keyword))
                .map_or_else(Condition::opaque, |value| Condition::parse(value, source)),
        },
        _ => return None,
    };
    Some(Constraint { name, kind })
}

fn parsed_condition(call: &Call<'_>, source: &[u8]) -> Option<Condition> {
    call.keyword(CONDITION)
        .map(|value| Condition::parse(value, source))
}

fn operator_classes(call: &Call<'_>, source: &[u8]) -> Vec<String> {
    call.keyword(OPERATOR_CLASSES)
        .map(|value| strings(value, source))
        .unwrap_or_default()
}

pub fn indexes(node: Node<'_>, source: &[u8]) -> Vec<Index> {
    elements(node)
        .into_iter()
        .filter_map(|element| index(element, source))
        .collect()
}

pub fn constraints(node: Node<'_>, source: &[u8]) -> Vec<Constraint> {
    elements(node)
        .into_iter()
        .filter_map(|element| constraint(element, source))
        .collect()
}
