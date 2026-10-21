use tree_sitter::Node;

use super::nodes::{
    ASSIGNMENT, COMMENT, IDENTIFIER, LEFT_FIELD, NAME_FIELD, PARENTHESIZED_TYPE, RIGHT_FIELD, THIS,
    VALUE_FIELD, VARIABLE_DECLARATOR, enclosed_children, first_named_child, without_parentheses,
};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;
use crate::syntax::named_children;

const PREDEFINED_TYPE: &str = "predefined_type";
const ANY: &str = "any";
const NON_NULL: &str = "non_null_expression";
const INTERFACE: &str = "interface_declaration";
const EXTENDS_CLAUSE: &str = "extends_type_clause";
const OBJECT_TYPE: &str = "object_type";
const INTERSECTION_TYPE: &str = "intersection_type";
const BODY_FIELD: &str = "body";
const SINGLE_EXTENDED: usize = 1;
const OPERAND_FIELDS: &[(&str, &str)] = &[
    ("binary_expression", "right"),
    ("unary_expression", "argument"),
    ("update_expression", "argument"),
    ("ternary_expression", "alternative"),
    ("assignment_expression", "right"),
    ("augmented_assignment_expression", "right"),
];
const PREFIX_KINDS: &[&str] = &["await_expression", "yield_expression", "type_assertion"];

pub fn explicit_any(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[PREDEFINED_TYPE])
        .filter(|predefined| context.text(*predefined) == ANY)
        .map(|predefined| report(EslintRule::NoExplicitAny, predefined, ANY))
        .collect()
}

pub fn non_null_assertions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[NON_NULL])
        .map(|assertion| report(EslintRule::NoNonNullAssertion, asserted_operand(assertion), ""))
        .collect()
}

fn asserted_operand(assertion: Node<'_>) -> Node<'_> {
    let mut operand = first_named_child(assertion).unwrap_or(assertion);
    while let Some(inner) = right_operand(operand) {
        operand = inner;
    }
    operand
}

fn right_operand(node: Node<'_>) -> Option<Node<'_>> {
    if let Some((_, field)) = OPERAND_FIELDS.iter().find(|(kind, _)| *kind == node.kind()) {
        return node.child_by_field_name(field);
    }
    if PREFIX_KINDS.contains(&node.kind()) {
        return named_children(node)
            .into_iter()
            .rev()
            .find(|child| child.kind() != COMMENT);
    }
    None
}

pub fn this_aliases(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let declared = context
        .of_kind(&[VARIABLE_DECLARATOR])
        .filter(|declarator| is_this(declarator.child_by_field_name(VALUE_FIELD)))
        .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD));
    let assigned = context
        .of_kind(&[ASSIGNMENT])
        .filter(|assignment| is_this(assignment.child_by_field_name(RIGHT_FIELD)))
        .filter_map(|assignment| assignment.child_by_field_name(LEFT_FIELD))
        .map(without_parentheses);
    declared
        .chain(assigned)
        .filter(|alias| alias.kind() == IDENTIFIER)
        .map(|alias| report(EslintRule::NoThisAlias, alias, context.text(alias)))
        .collect()
}

fn is_this(value: Option<Node<'_>>) -> bool {
    value.is_some_and(|value| without_parentheses(value).kind() == THIS)
}

pub fn empty_object_types(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let interfaces = context
        .of_kind(&[INTERFACE])
        .filter(|interface| is_empty_interface(*interface))
        .filter_map(|interface| interface.child_by_field_name(NAME_FIELD))
        .map(|name| report(EslintRule::NoEmptyObjectType, name, context.text(name)));
    let literals = context
        .of_kind(&[OBJECT_TYPE])
        .filter(|literal| members(*literal).is_empty() && !is_in_intersection(*literal))
        .map(|literal| report(EslintRule::NoEmptyObjectType, literal, ""));
    interfaces.chain(literals).collect()
}

fn is_empty_interface(interface: Node<'_>) -> bool {
    let extended = named_children(interface)
        .into_iter()
        .filter(|child| child.kind() == EXTENDS_CLAUSE)
        .map(|clause| members(clause).len())
        .sum::<usize>();
    extended <= SINGLE_EXTENDED
        && interface
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|body| members(body).is_empty())
}

fn members(node: Node<'_>) -> Vec<Node<'_>> {
    enclosed_children(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn is_in_intersection(literal: Node<'_>) -> bool {
    let mut parent = literal.parent();
    while let Some(current) = parent.filter(|current| current.kind() == PARENTHESIZED_TYPE) {
        parent = current.parent();
    }
    parent.is_some_and(|parent| parent.kind() == INTERSECTION_TYPE)
}
