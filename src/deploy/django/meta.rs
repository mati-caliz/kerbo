use tree_sitter::Node;

use super::model::{Constraint, ConstraintKind, Index};
use super::values::{elements, is_sequence, strings, text};
use crate::deploy::python_calls::Call;

const UNIQUE_CONSTRAINT: &str = "UniqueConstraint";
const CHECK_CONSTRAINT: &str = "CheckConstraint";
const INDEX_ONLY_UNIQUE_KEYWORDS: &[&str] = &["condition", "include", "opclasses", "nulls_distinct"];
const CONDITION: &str = "condition";
const EXPRESSION_PLACEHOLDER: &str = "(true)";

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
        conditional: call.keyword(CONDITION).is_some(),
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
            ConstraintKind::Unique { fields, by_index }
        }
        CHECK_CONSTRAINT => ConstraintKind::Check,
        _ => return None,
    };
    Some(Constraint { name, kind })
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
