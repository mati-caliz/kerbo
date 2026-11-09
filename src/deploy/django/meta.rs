use tree_sitter::Node;

use super::condition::Condition;
use super::constants::Constants;
use super::expression::Expression;
use super::model::{Constraint, ConstraintKind, Index};
use super::values::{boolean, elements, is_sequence, raw, strings, text};
use crate::deploy::python_calls::Call;

const UNIQUE_CONSTRAINT: &str = "UniqueConstraint";
const CHECK_CONSTRAINT: &str = "CheckConstraint";
const INDEX_ONLY_UNIQUE_KEYWORDS: &[&str] = &["condition", "include", "opclasses"];
const NULLS_DISTINCT: &str = "nulls_distinct";
const CHECK_KEYWORDS: &[&str] = &["condition", "check"];
const CONDITION: &str = "condition";
const STORAGE_PARAMETERS: &[&str] = &[
    "buffering",
    "fillfactor",
    "deduplicate_items",
    "fastupdate",
    "gin_pending_list_limit",
    "autosummarize",
    "pages_per_range",
];
const STORAGE_ON: &str = "on";
const STORAGE_OFF: &str = "off";
const NONE: &str = "none";
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

pub fn index<'tree>(node: Node<'tree>, constants: Constants<'tree>) -> Option<Index> {
    let source = constants.source;
    let call = Call::parse(node, source)?;
    let name = call.keyword("name").and_then(|value| text(value, source))?;
    Some(Index {
        name,
        fields: call
            .keyword("fields")
            .map(|value| strings(value, source))
            .unwrap_or_default(),
        expressions: expressions(&call, source),
        condition: parsed_condition(&call, constants),
        method: INDEX_METHODS
            .iter()
            .find(|(class, _)| *class == call.method)
            .map(|(_, method)| *method),
        opclasses: operator_classes(&call, source),
        storage: storage_parameters(&call, source),
    })
}

pub fn constraint<'tree>(node: Node<'tree>, constants: Constants<'tree>) -> Option<Constraint> {
    let source = constants.source;
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
                expressions: expressions(&call, source),
                by_index,
                condition: parsed_condition(&call, constants),
                opclasses: operator_classes(&call, source),
                nulls_distinct: call.keyword(NULLS_DISTINCT).and_then(boolean),
            }
        }
        CHECK_CONSTRAINT => ConstraintKind::Check {
            condition: CHECK_KEYWORDS
                .iter()
                .find_map(|keyword| call.keyword(keyword))
                .map_or_else(Condition::opaque, |value| Condition::parse(value, constants)),
        },
        _ => return None,
    };
    Some(Constraint { name, kind })
}

fn expressions(call: &Call<'_>, source: &[u8]) -> Vec<Expression> {
    call.all_positional()
        .into_iter()
        .map(|argument| Expression::parse(argument, source))
        .collect()
}

fn storage_parameters(call: &Call<'_>, source: &[u8]) -> Vec<String> {
    STORAGE_PARAMETERS
        .iter()
        .filter_map(|parameter| {
            let value = call.keyword(parameter)?;
            let rendered = match boolean(value) {
                Some(true) => STORAGE_ON.to_owned(),
                Some(false) => STORAGE_OFF.to_owned(),
                None if value.kind() == NONE => return None,
                None => raw(value, source),
            };
            Some(format!("{parameter} = {rendered}"))
        })
        .collect()
}

fn parsed_condition<'tree>(call: &Call<'tree>, constants: Constants<'tree>) -> Option<Condition> {
    call.keyword(CONDITION)
        .map(|value| Condition::parse(value, constants))
}

fn operator_classes(call: &Call<'_>, source: &[u8]) -> Vec<String> {
    call.keyword(OPERATOR_CLASSES)
        .map(|value| strings(value, source))
        .unwrap_or_default()
}

pub fn indexes<'tree>(node: Node<'tree>, constants: Constants<'tree>) -> Vec<Index> {
    elements(node)
        .into_iter()
        .filter_map(|element| index(element, constants))
        .collect()
}

pub fn constraints<'tree>(node: Node<'tree>, constants: Constants<'tree>) -> Vec<Constraint> {
    elements(node)
        .into_iter()
        .filter_map(|element| constraint(element, constants))
        .collect()
}
