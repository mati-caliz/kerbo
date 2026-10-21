use std::collections::HashMap;

use tree_sitter::Node;

use super::nodes::{
    BODY_FIELD, CONDITION_FIELD, ELSE, IF, KEY_FIELD, NAME_FIELD, NUMBER, STRING, SWITCH_CASE,
    TEMPLATE_STRING, VALUE_FIELD, first_named_child, text, without_parentheses,
};
use super::strings::{number_key, static_string};
use super::tokens::{AND, OR, equal_tokens, estree_kind, logical_operands};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;
use crate::syntax::named_children;

const SWITCH: &str = "switch_statement";
const OBJECT: &str = "object";
const PAIR: &str = "pair";
const SHORTHAND: &str = "shorthand_property_identifier";
const METHOD: &str = "method_definition";
const COMPUTED: &str = "computed_property_name";
const PROPERTY_IDENTIFIER: &str = "property_identifier";
const GETTER: &str = "get";
const SETTER: &str = "set";
const PROTO: &str = "__proto__";
const ALTERNATIVE_FIELD: &str = "alternative";
const LITERAL_WORDS: &[&str] = &["true", "false", "null", "regex"];

pub fn duplicate_cases(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for switch in context.of_kind(&[SWITCH]) {
        let Some(body) = switch.child_by_field_name(BODY_FIELD) else {
            continue;
        };
        let mut previous: Vec<Node<'_>> = Vec::new();
        for case in named_children(body)
            .into_iter()
            .filter(|case| case.kind() == SWITCH_CASE)
        {
            let Some(test) = case.child_by_field_name(VALUE_FIELD) else {
                continue;
            };
            if previous
                .iter()
                .any(|earlier| equal(*earlier, test, context.source))
            {
                findings.push(report(EslintRule::NoDuplicateCase, case, ""));
            } else {
                previous.push(test);
            }
        }
    }
    findings
}

fn equal(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    estree_kind(left, source) == estree_kind(right, source) && equal_tokens(left, right, source)
}

fn equal_conditions(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    if estree_kind(left, source) != estree_kind(right, source) {
        return false;
    }
    for wanted in [OR, AND] {
        if let (Some((left_a, left_b)), Some((right_a, right_b))) = (
            logical_operands(left, wanted, source),
            logical_operands(right, wanted, source),
        ) {
            return (equal_conditions(left_a, right_a, source) && equal_conditions(left_b, right_b, source))
                || (equal_conditions(left_a, right_b, source) && equal_conditions(left_b, right_a, source));
        }
    }
    equal_tokens(left, right, source)
}

fn split<'tree>(node: Node<'tree>, wanted: &str, source: &[u8]) -> Vec<Node<'tree>> {
    logical_operands(node, wanted, source).map_or_else(
        || vec![without_parentheses(node)],
        |(left, right)| {
            let mut parts = split(left, wanted, source);
            parts.extend(split(right, wanted, source));
            parts
        },
    )
}

fn or_of_ands<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Vec<Node<'tree>>> {
    split(node, OR, source)
        .into_iter()
        .map(|operand| split(operand, AND, source))
        .collect()
}

fn is_subset(smaller: &[Node<'_>], larger: &[Node<'_>], source: &[u8]) -> bool {
    smaller
        .iter()
        .all(|left| larger.iter().any(|right| equal_conditions(*left, *right, source)))
}

fn if_test(statement: Node<'_>) -> Option<Node<'_>> {
    statement
        .child_by_field_name(CONDITION_FIELD)
        .and_then(first_named_child)
        .map(without_parentheses)
}

fn enclosing_if(statement: Node<'_>) -> Option<Node<'_>> {
    let clause = statement.parent().filter(|clause| clause.kind() == ELSE)?;
    let parent = clause.parent().filter(|parent| parent.kind() == IF)?;
    let alternative = parent.child_by_field_name(ALTERNATIVE_FIELD)?;
    (alternative.id() == clause.id()).then_some(parent)
}

pub fn duplicate_else_ifs(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[IF])
        .filter_map(|statement| {
            let test = if_test(statement)?;
            is_covered(statement, test, context.source).then(|| report(EslintRule::NoDupeElseIf, test, ""))
        })
        .collect()
}

fn is_covered(statement: Node<'_>, test: Node<'_>, source: &[u8]) -> bool {
    let mut conditions = vec![test];
    if logical_operands(test, AND, source).is_some() {
        conditions.extend(split(test, AND, source));
    }
    let mut remaining: Vec<Vec<Vec<Node<'_>>>> = conditions
        .into_iter()
        .map(|condition| or_of_ands(condition, source))
        .collect();
    let mut current = statement;
    while let Some(parent) = enclosing_if(current) {
        current = parent;
        let Some(parent_test) = if_test(parent) else {
            return false;
        };
        let covering = or_of_ands(parent_test, source);
        for operands in &mut remaining {
            operands.retain(|operand| !covering.iter().any(|cover| is_subset(cover, operand, source)));
        }
        if remaining.iter().any(Vec::is_empty) {
            return true;
        }
    }
    false
}

#[derive(Debug, Default, Clone, Copy)]
struct Accessors {
    getter: bool,
    setter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyKind {
    Init,
    Get,
    Set,
}

pub fn duplicate_keys(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for object in context.of_kind(&[OBJECT]) {
        let mut defined: HashMap<String, Accessors> = HashMap::new();
        for property in named_children(object) {
            let Some((key, name, kind)) = property_key(property, context.source) else {
                continue;
            };
            let entry = defined.entry(name.clone()).or_default();
            let reads = kind != PropertyKind::Set && entry.getter;
            let writes = kind != PropertyKind::Get && entry.setter;
            if reads || writes {
                findings.push(report(EslintRule::NoDupeKeys, key, &name));
            }
            entry.getter |= kind != PropertyKind::Set;
            entry.setter |= kind != PropertyKind::Get;
        }
    }
    findings
}

fn property_key<'tree>(property: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, String, PropertyKind)> {
    match property.kind() {
        SHORTHAND => Some((property, text(property, source).to_owned(), PropertyKind::Init)),
        PAIR => {
            let key = property.child_by_field_name(KEY_FIELD)?;
            let (position, name) = key_name(key, source)?;
            let is_proto = key.kind() != COMPUTED && name == PROTO;
            (!is_proto).then_some((position, name, PropertyKind::Init))
        }
        METHOD => {
            let key = property.child_by_field_name(NAME_FIELD)?;
            let (position, name) = key_name(key, source)?;
            Some((position, name, accessor_kind(property, key, source)))
        }
        _ => None,
    }
}

fn accessor_kind(method: Node<'_>, key: Node<'_>, source: &[u8]) -> PropertyKind {
    let mut cursor = method.walk();
    let keyword = method
        .children(&mut cursor)
        .take_while(|child| child.id() != key.id())
        .find(|child| !child.is_named() && [GETTER, SETTER].contains(&text(*child, source)));
    match keyword.map(|keyword| text(keyword, source)) {
        Some(GETTER) => PropertyKind::Get,
        Some(SETTER) => PropertyKind::Set,
        _ => PropertyKind::Init,
    }
}

fn key_name<'tree>(key: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, String)> {
    let (position, value) = if key.kind() == COMPUTED {
        let inner = without_parentheses(first_named_child(key)?);
        (inner, literal_value(inner, source)?)
    } else {
        (key, literal_value(key, source)?)
    };
    Some((position, value))
}

fn literal_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        PROPERTY_IDENTIFIER => Some(text(node, source).to_owned()),
        STRING | TEMPLATE_STRING => static_string(node, source),
        NUMBER => number_key(node, source),
        kind if LITERAL_WORDS.contains(&kind) => Some(text(node, source).to_owned()),
        _ => None,
    }
}
