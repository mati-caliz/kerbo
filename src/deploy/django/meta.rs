use tree_sitter::Node;

use super::condition::Condition;
use super::expression::{Context, Expression};
use super::model::{Constraint, ConstraintKind, Index};
use super::values::{boolean, elements, is_sequence, raw, strings, text};
use crate::deploy::python_calls::Call;

const UNIQUE_CONSTRAINT: &str = "UniqueConstraint";
const CHECK_CONSTRAINT: &str = "CheckConstraint";
const EXCLUSION_CONSTRAINT: &str = "ExclusionConstraint";
const DEFAULT_EXCLUSION_INDEX_TYPE: &str = "GIST";
const INDEX_TYPE: &str = "index_type";
const EXPRESSIONS: &str = "expressions";
const INCLUDE: &str = "include";
const RANGE_OPERATORS: &[(&str, &str)] = &[
    ("EQUAL", "="),
    ("NOT_EQUAL", "<>"),
    ("CONTAINS", "@>"),
    ("CONTAINED_BY", "<@"),
    ("OVERLAPS", "&&"),
    ("FULLY_LT", "<<"),
    ("FULLY_GT", ">>"),
    ("NOT_LT", "&>"),
    ("NOT_GT", "&<"),
    ("ADJACENT_TO", "-|-"),
];
const INDEX_ONLY_UNIQUE_KEYWORDS: &[&str] = &["condition", "include", "opclasses"];
const NULLS_DISTINCT: &str = "nulls_distinct";
const DEFERRABLE: &str = "deferrable";
const DEFERRABLE_MODES: &[(&str, &str)] = &[
    ("DEFERRED", " DEFERRABLE INITIALLY DEFERRED"),
    ("IMMEDIATE", " DEFERRABLE INITIALLY IMMEDIATE"),
];
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

pub fn index<'tree>(node: Node<'tree>, context: Context<'tree>) -> Option<Index> {
    let source = context.constants.source;
    let call = Call::parse(node, source)?;
    let name = call.keyword("name").and_then(|value| text(value, source))?;
    Some(Index {
        name,
        fields: call
            .keyword("fields")
            .map(|value| strings(value, source))
            .unwrap_or_default(),
        expressions: expressions(&call, context),
        include: included(&call, source),
        condition: parsed_condition(&call, context),
        method: INDEX_METHODS
            .iter()
            .find(|(class, _)| *class == call.method)
            .map(|(_, method)| *method),
        opclasses: operator_classes(&call, source),
        storage: storage_parameters(&call, source),
    })
}

pub fn constraint<'tree>(node: Node<'tree>, context: Context<'tree>) -> Option<Constraint> {
    let source = context.constants.source;
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
                expressions: expressions(&call, context),
                include: included(&call, source),
                by_index,
                condition: parsed_condition(&call, context),
                opclasses: operator_classes(&call, source),
                nulls_distinct: call.keyword(NULLS_DISTINCT).and_then(boolean),
                deferrable: deferrable(&call, source),
            }
        }
        CHECK_CONSTRAINT => ConstraintKind::Check {
            condition: CHECK_KEYWORDS
                .iter()
                .find_map(|keyword| call.keyword(keyword))
                .map_or_else(Condition::opaque, |value| {
                    Condition::parse(value, context.constants)
                }),
        },
        EXCLUSION_CONSTRAINT => ConstraintKind::Exclusion {
            expressions: exclusion_expressions(&call, context),
            index_type: call
                .keyword(INDEX_TYPE)
                .and_then(|value| text(value, source))
                .map_or_else(
                    || DEFAULT_EXCLUSION_INDEX_TYPE.to_owned(),
                    |index_type| index_type.to_uppercase(),
                ),
            condition: parsed_condition(&call, context),
            include: included(&call, source),
            deferrable: deferrable(&call, source),
        },
        _ => return None,
    };
    Some(Constraint { name, kind })
}

fn deferrable(call: &Call<'_>, source: &[u8]) -> Option<&'static str> {
    let value = call.keyword(DEFERRABLE)?;
    let member = match value.kind() {
        "attribute" => value
            .child_by_field_name("attribute")?
            .utf8_text(source)
            .ok()
            .map(str::to_owned),
        "subscript" => value
            .child_by_field_name("subscript")
            .and_then(|key| text(key, source)),
        _ => None,
    }?;
    DEFERRABLE_MODES
        .iter()
        .find(|(mode, _)| *mode == member)
        .map(|(_, clause)| *clause)
}

fn expressions<'tree>(call: &Call<'tree>, context: Context<'tree>) -> Vec<Expression> {
    call.all_positional()
        .into_iter()
        .map(|argument| Expression::parse(argument, context))
        .collect()
}

fn exclusion_expressions<'tree>(call: &Call<'tree>, context: Context<'tree>) -> Vec<(Expression, String)> {
    let source = context.constants.source;
    call.keyword(EXPRESSIONS)
        .map(|value| elements(context.constants.resolve(value)))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|pair| {
            let parts = elements(pair);
            let operator = parts
                .get(1)
                .and_then(|operator| exclusion_operator(context.constants.resolve(*operator), source))?;
            Some((Expression::parse(*parts.first()?, context), operator))
        })
        .collect()
}

fn exclusion_operator(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(operator) = text(node, source) {
        return Some(operator);
    }
    let member = node.child_by_field_name("attribute")?.utf8_text(source).ok()?;
    RANGE_OPERATORS
        .iter()
        .find(|(name, _)| *name == member)
        .map(|(_, operator)| (*operator).to_owned())
}

fn included(call: &Call<'_>, source: &[u8]) -> Vec<String> {
    call.keyword(INCLUDE)
        .map(|value| strings(value, source))
        .unwrap_or_default()
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

fn parsed_condition<'tree>(call: &Call<'tree>, context: Context<'tree>) -> Option<Condition> {
    call.keyword(CONDITION)
        .map(|value| Condition::parse(value, context.constants))
}

fn operator_classes(call: &Call<'_>, source: &[u8]) -> Vec<String> {
    call.keyword(OPERATOR_CLASSES)
        .map(|value| strings(value, source))
        .unwrap_or_default()
}

pub fn indexes<'tree>(node: Node<'tree>, context: Context<'tree>) -> Vec<Index> {
    elements(node)
        .into_iter()
        .filter_map(|element| index(element, context))
        .collect()
}

pub fn constraints<'tree>(node: Node<'tree>, context: Context<'tree>) -> Vec<Constraint> {
    elements(node)
        .into_iter()
        .filter_map(|element| constraint(element, context))
        .collect()
}
