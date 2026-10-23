use tree_sitter::Node;

use super::arrays::elements_with_holes;
use super::members::{static_key_name, static_property_name, static_string_value};
use super::nodes::{
    ASSIGNMENT, ASSIGNMENTS, COMMENT, IDENTIFIER, INDEX_FIELD, KEY_FIELD, LEFT_FIELD, MEMBER, NUMBER,
    OBJECT_FIELD, PROPERTY_FIELD, RIGHT_FIELD, STRING, SUBSCRIPT, THIS, VALUE_FIELD, operator, text,
    without_parentheses,
};
use super::strings::numeric_text;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const SELF_ASSIGNING_OPERATORS: &[&str] = &["&&=", "||=", "??="];
const UNDEFINED: &str = "undefined";
const SUPER: &str = "super";
const PRIVATE_NAME: &str = "private_property_identifier";
const ARRAY_PATTERN: &str = "array_pattern";
const ARRAY: &str = "array";
const OBJECT_PATTERN: &str = "object_pattern";
const OBJECT: &str = "object";
const REST: &str = "rest_pattern";
const SPREAD: &str = "spread_element";
const SHORTHAND_PATTERN: &str = "shorthand_property_identifier_pattern";
const PAIR_PATTERN: &str = "pair_pattern";
const SHORTHAND: &str = "shorthand_property_identifier";
const PAIR: &str = "pair";
const REGEX: &str = "regex";
const KEYWORD_LITERALS: &[&str] = &["true", "false", "null"];
const BIGINT_SUFFIX: char = 'n';

// `no-self-assign` con `props: true`: `a = a`, `[a, b] = [a, b]`, `({a} = {a})`, `x.y = x.y`.
pub fn self_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut found = Vec::new();
    for assignment in context.of_kind(ASSIGNMENTS) {
        let is_plain = assignment.kind() == ASSIGNMENT;
        if !is_plain && !SELF_ASSIGNING_OPERATORS.contains(&operator(assignment, context.source)) {
            continue;
        }
        if let (Some(left), Some(right)) = (
            assignment.child_by_field_name(LEFT_FIELD),
            assignment.child_by_field_name(RIGHT_FIELD),
        ) {
            each_self_assignment(Some(left), Some(right), context.source, &mut found);
        }
    }
    found
        .into_iter()
        .map(|right| report(EslintRule::NoSelfAssign, right, text(right, context.source)))
        .collect()
}

fn each_self_assignment<'tree>(
    left: Option<Node<'tree>>,
    right: Option<Node<'tree>>,
    source: &[u8],
    found: &mut Vec<Node<'tree>>,
) {
    let (Some(left), Some(right)) = (left.map(without_parentheses), right.map(without_parentheses)) else {
        return;
    };
    match (left.kind(), right.kind()) {
        (left_kind, right_kind) if is_identifier(left_kind) && is_identifier(right_kind) => {
            if text(left, source) == text(right, source) {
                found.push(right);
            }
        }
        (ARRAY_PATTERN, ARRAY) => each_element(left, right, source, found),
        (REST, SPREAD) => each_self_assignment(left.named_child(0), right.named_child(0), source, found),
        (OBJECT_PATTERN, OBJECT) => each_property(left, right, source, found),
        (left_kind, right_kind)
            if is_member(left_kind) && is_member(right_kind) && is_same_reference(left, right, source) =>
        {
            found.push(right);
        }
        _ => {}
    }
}

fn each_element<'tree>(left: Node<'tree>, right: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    let left_elements = elements_with_holes(left);
    let right_elements = elements_with_holes(right);
    let last_right = right_elements.len().saturating_sub(1);
    for (index, (left_element, right_element)) in left_elements.iter().zip(&right_elements).enumerate() {
        if left_element.is_some_and(|element| element.kind() == REST) && index < last_right {
            break;
        }
        each_self_assignment(*left_element, *right_element, source, found);
        if right_element.is_some_and(|element| element.kind() == SPREAD) {
            break;
        }
    }
}

fn each_property<'tree>(left: Node<'tree>, right: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    let right_properties = properties(right);
    if right_properties.is_empty() {
        return;
    }
    let start = right_properties
        .iter()
        .rposition(|property| property.kind() == SPREAD)
        .map_or(0, |last_spread| last_spread.saturating_add(1));
    for left_property in properties(left) {
        for right_property in right_properties.iter().skip(start) {
            let names = property_name(left_property, source).zip(property_name(*right_property, source));
            if names.is_some_and(|(left_name, right_name)| left_name == right_name) {
                each_self_assignment(
                    property_value(left_property),
                    property_value(*right_property),
                    source,
                    found,
                );
            }
        }
    }
}

fn properties(object: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = object.walk();
    object
        .named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn property_name(property: Node<'_>, source: &[u8]) -> Option<String> {
    match property.kind() {
        SHORTHAND_PATTERN | SHORTHAND => Some(text(property, source).to_owned()),
        PAIR_PATTERN | PAIR => property
            .child_by_field_name(KEY_FIELD)
            .and_then(|key| static_key_name(key, source)),
        _ => None,
    }
}

fn property_value(property: Node<'_>) -> Option<Node<'_>> {
    match property.kind() {
        SHORTHAND_PATTERN | SHORTHAND => Some(property),
        PAIR_PATTERN | PAIR => property.child_by_field_name(VALUE_FIELD),
        _ => None,
    }
}

fn is_identifier(kind: &str) -> bool {
    [IDENTIFIER, UNDEFINED, SHORTHAND_PATTERN, SHORTHAND].contains(&kind)
}

fn is_member(kind: &str) -> bool {
    [MEMBER, SUBSCRIPT].contains(&kind)
}

// `isSameReference`: los dos lados nombran lo mismo (`this.a.b` y `this["a"].b`).
fn is_same_reference(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let left = without_parentheses(left);
    let right = without_parentheses(right);
    if is_member(left.kind()) && is_member(right.kind()) {
        return is_same_member(left, right, source);
    }
    if left.kind() != right.kind() && !(is_literal(left.kind()) && is_literal(right.kind())) {
        return false;
    }
    match left.kind() {
        SUPER | THIS => true,
        IDENTIFIER | UNDEFINED | PRIVATE_NAME => text(left, source) == text(right, source),
        kind if is_literal(kind) => equal_literal_value(left, right, source),
        _ => false,
    }
}

fn is_same_member(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let objects_match = left
        .child_by_field_name(OBJECT_FIELD)
        .zip(right.child_by_field_name(OBJECT_FIELD))
        .is_some_and(|(left_object, right_object)| is_same_reference(left_object, right_object, source));
    if let Some(name) = static_property_name(left, source) {
        return objects_match && static_property_name(right, source).is_some_and(|other| other == name);
    }
    left.kind() == right.kind()
        && objects_match
        && member_property(left)
            .zip(member_property(right))
            .is_some_and(|(left_property, right_property)| {
                is_same_reference(left_property, right_property, source)
            })
}

fn member_property(member: Node<'_>) -> Option<Node<'_>> {
    let field = if member.kind() == MEMBER {
        PROPERTY_FIELD
    } else {
        INDEX_FIELD
    };
    member.child_by_field_name(field)
}

fn is_literal(kind: &str) -> bool {
    [STRING, NUMBER, REGEX].contains(&kind) || KEYWORD_LITERALS.contains(&kind)
}

// `equalLiteralValue`: misma clase de literal y mismo valor (`1` y `1.0` son iguales).
fn equal_literal_value(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    if left.kind() == REGEX || right.kind() == REGEX {
        return left.kind() == right.kind() && text(left, source) == text(right, source);
    }
    let is_bigint = |node: Node<'_>| node.kind() == NUMBER && text(node, source).ends_with(BIGINT_SUFFIX);
    if is_bigint(left) || is_bigint(right) {
        return is_bigint(left)
            && is_bigint(right)
            && numeric_text(text(left, source)) == numeric_text(text(right, source));
    }
    let left_value = static_string_value(left, source);
    left.kind() == right.kind() && left_value.is_some() && left_value == static_string_value(right, source)
}
