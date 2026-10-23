use std::collections::HashMap;

use tree_sitter::Node;

use super::nodes::{LEFT_FIELD, NAME_FIELD, VALUE_FIELD};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const CLASS_BODY: &str = "class_body";
const PRIVATE_NAME: &str = "private_property_identifier";
const METHOD: &str = "method_definition";
const FIELDS: &[&str] = &["field_definition", "public_field_definition"];
const PROPERTY_FIELD: &str = "property";
const ACCESSOR_KEYWORDS: &[&str] = &["get", "set"];
const ASSIGNMENT: &str = "assignment_expression";
const AUGMENTED_ASSIGNMENT: &str = "augmented_assignment_expression";
const WRITE_TARGETS: &[&str] = &["for_in_statement", "assignment_pattern"];
const UPDATE: &str = "update_expression";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const PAIR_PATTERN: &str = "pair_pattern";
const OBJECT_PATTERN: &str = "object_pattern";
const NOT_READ_PARENTS: &[&str] = &["rest_pattern", "array_pattern"];

struct Member<'tree> {
    key: Node<'tree>,
    accessor: bool,
    used: bool,
}

// `no-unused-private-class-members`: un `#campo` o `#metodo` que la clase nunca lee.
pub fn unused_private_members(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let mut classes: HashMap<usize, HashMap<String, Member<'_>>> = context
        .of_kind(&[CLASS_BODY])
        .map(|body| (body.id(), declared_members(body, context)))
        .collect();
    for name in context.of_kind(&[PRIVATE_NAME]) {
        let text = context.text(name);
        let Some(body) = enclosing_bodies(name).map(|body| body.id()).find(|body| {
            classes
                .get(body)
                .is_some_and(|members| members.contains_key(text))
        }) else {
            continue;
        };
        let Some(member) = classes.get_mut(&body).and_then(|members| members.get_mut(text)) else {
            continue;
        };
        if member.used || member.key.id() == name.id() {
            continue;
        }
        if member.accessor || is_read(name) {
            member.used = true;
        }
    }
    let mut found: Vec<LintFinding> = classes
        .into_values()
        .flat_map(HashMap::into_values)
        .filter(|member| !member.used)
        .map(|member| {
            report(
                EslintRule::NoUnusedPrivateClassMembers,
                member.key,
                context.text(member.key),
            )
        })
        .collect();
    found.sort_by_key(|finding| (finding.line, finding.column));
    found
}

fn declared_members<'tree>(
    body: Node<'tree>,
    context: &Context<'tree, '_>,
) -> HashMap<String, Member<'tree>> {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter_map(|member| {
            let field = if member.kind() == METHOD {
                NAME_FIELD
            } else {
                PROPERTY_FIELD
            };
            let is_member = member.kind() == METHOD || FIELDS.contains(&member.kind());
            let key = member
                .child_by_field_name(field)
                .filter(|key| is_member && key.kind() == PRIVATE_NAME)?;
            let mut inner = member.walk();
            let accessor = member.kind() == METHOD
                && member
                    .children(&mut inner)
                    .any(|child| !child.is_named() && ACCESSOR_KEYWORDS.contains(&child.kind()));
            Some((
                context.text(key).to_owned(),
                Member {
                    key,
                    accessor,
                    used: false,
                },
            ))
        })
        .collect()
}

fn enclosing_bodies(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent).filter(|ancestor| ancestor.kind() == CLASS_BODY)
}

// Lo que no cuenta como lectura: asignarle, incrementarlo como sentencia o desestructurar hacia él.
fn is_read(name: Node<'_>) -> bool {
    let Some(access) = name.parent() else {
        return true;
    };
    let Some(wrapper) = access.parent() else {
        return true;
    };
    if is_write_only(access, wrapper) {
        return false;
    }
    let statement_parent = wrapper.parent().map(|parent| parent.kind());
    if wrapper.kind() == UPDATE && statement_parent == Some(EXPRESSION_STATEMENT) {
        return false;
    }
    if wrapper.kind() == PAIR_PATTERN
        && statement_parent == Some(OBJECT_PATTERN)
        && wrapper
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| value.id() == access.id())
    {
        return false;
    }
    !NOT_READ_PARENTS.contains(&wrapper.kind())
}

// `isWriteOnlyAssignment`.
fn is_write_only(access: Node<'_>, wrapper: Node<'_>) -> bool {
    let kind = wrapper.kind();
    let is_target = [ASSIGNMENT, AUGMENTED_ASSIGNMENT].contains(&kind) || WRITE_TARGETS.contains(&kind);
    if !is_target
        || wrapper
            .child_by_field_name(LEFT_FIELD)
            .is_none_or(|left| left.id() != access.id())
    {
        return false;
    }
    if kind == AUGMENTED_ASSIGNMENT {
        return wrapper
            .parent()
            .is_some_and(|parent| parent.kind() == EXPRESSION_STATEMENT);
    }
    true
}
