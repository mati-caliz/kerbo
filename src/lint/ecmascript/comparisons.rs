use tree_sitter::Node;

use super::constants::{Constants, UNARY, last_expression};
use super::members::static_property_name;
use super::nodes::{
    BINARY, IDENTIFIER, LEFT_FIELD, MEMBER, NUMBER, OBJECT_FIELD, RIGHT_FIELD, STRING, SUBSCRIPT,
    SWITCH_CASE, TEMPLATE_STRING, VALUE_FIELD, operator, parent_without_parentheses, without_parentheses,
};
use super::strings::{has_substitutions, numeric_text, static_string};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const TYPEOF: &str = "typeof";
const ARGUMENT_FIELD: &str = "argument";
const VALID_TYPES: &[&str] = &[
    "symbol",
    "undefined",
    "object",
    "boolean",
    "number",
    "string",
    "function",
    "bigint",
];
const EQUALITY: &[&str] = &["==", "===", "!=", "!=="];
const RELATIONAL: &[&str] = &["<", ">", "<=", ">=", "==", "===", "!=", "!=="];
const LITERALS: &[&str] = &[STRING, NUMBER, "regex", "true", "false", "null"];
const UNDEFINED: &str = "undefined";
const MINUS: &str = "-";
const ZERO: &str = "0";
const BIGINT_SUFFIX: char = 'n';
const NAN: &str = "NaN";
const NUMBER_GLOBAL: &str = "Number";
const SEQUENCE: &str = "sequence_expression";
const SWITCH: &str = "switch_statement";

// `valid-typeof`: `typeof x` comparado con algo que `typeof` nunca devuelve.
pub fn invalid_typeof_comparisons(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[UNARY])
        .filter(|unary| operator(*unary, context.source) == TYPEOF)
        .filter_map(|unary| compared_sibling(unary, context))
        .filter(|sibling| is_invalid_type(*sibling, &constants, context))
        .map(|sibling| report(EslintRule::ValidTypeof, sibling, ""))
        .collect()
}

fn compared_sibling<'tree>(unary: Node<'tree>, context: &Context<'tree, '_>) -> Option<Node<'tree>> {
    let parent = parent_without_parentheses(unary)
        .filter(|parent| parent.kind() == BINARY && EQUALITY.contains(&operator(*parent, context.source)))?;
    let left = without_parentheses(parent.child_by_field_name(LEFT_FIELD)?);
    let right = without_parentheses(parent.child_by_field_name(RIGHT_FIELD)?);
    Some(if left.id() == unary.id() { right } else { left })
}

fn is_invalid_type(sibling: Node<'_>, constants: &Constants<'_, '_, '_>, context: &Context<'_, '_>) -> bool {
    match sibling.kind() {
        STRING => {
            static_string(sibling, context.source).is_none_or(|value| !VALID_TYPES.contains(&value.as_str()))
        }
        TEMPLATE_STRING if !has_substitutions(sibling) => {
            static_string(sibling, context.source).is_none_or(|value| !VALID_TYPES.contains(&value.as_str()))
        }
        kind if LITERALS.contains(&kind) => true,
        IDENTIFIER | UNDEFINED => constants.is_global_undefined(sibling),
        _ => false,
    }
}

// `no-compare-neg-zero`: una comparación contra `-0`, que no distingue de `0`.
pub fn negative_zero_comparisons(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[BINARY])
        .filter(|binary| RELATIONAL.contains(&operator(*binary, context.source)))
        .filter(|binary| {
            [LEFT_FIELD, RIGHT_FIELD]
                .iter()
                .filter_map(|field| binary.child_by_field_name(field))
                .any(|side| is_negative_zero(without_parentheses(side), context))
        })
        .map(|binary| report(EslintRule::NoCompareNegZero, binary, ""))
        .collect()
}

fn is_negative_zero(node: Node<'_>, context: &Context<'_, '_>) -> bool {
    node.kind() == UNARY
        && operator(node, context.source) == MINUS
        && node
            .child_by_field_name(ARGUMENT_FIELD)
            .map(without_parentheses)
            .filter(|argument| argument.kind() == NUMBER)
            .is_some_and(|argument| {
                let raw = context.text(argument);
                !raw.ends_with(BIGINT_SUFFIX) && numeric_text(raw).is_some_and(|value| value == ZERO)
            })
}

// `use-isnan`: una comparación o un `switch` contra `NaN`, que nunca es igual a nada.
pub fn nan_comparisons(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    let binaries = context
        .of_kind(&[BINARY])
        .filter(|binary| RELATIONAL.contains(&operator(*binary, context.source)))
        .filter(|binary| {
            [LEFT_FIELD, RIGHT_FIELD]
                .iter()
                .filter_map(|field| binary.child_by_field_name(field))
                .any(|side| is_nan(side, &constants, context))
        });
    let switches = context.of_kind(&[SWITCH]).filter(|switch| {
        switch
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| is_nan(value, &constants, context))
    });
    let cases = context.of_kind(&[SWITCH_CASE]).filter(|case| {
        case.child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| is_nan(value, &constants, context))
    });
    binaries
        .chain(switches)
        .chain(cases)
        .map(|node| report(EslintRule::UseIsnan, node, NAN))
        .collect()
}

fn is_nan(node: Node<'_>, constants: &Constants<'_, '_, '_>, context: &Context<'_, '_>) -> bool {
    let mut node = without_parentheses(node);
    if node.kind() == SEQUENCE {
        match last_expression(node) {
            Some(last) => node = without_parentheses(last),
            None => return false,
        }
    }
    match node.kind() {
        IDENTIFIER => context.text(node) == NAN && constants.is_global(node),
        MEMBER | SUBSCRIPT => {
            static_property_name(node, context.source).is_some_and(|name| name == NAN)
                && node
                    .child_by_field_name(OBJECT_FIELD)
                    .map(without_parentheses)
                    .is_some_and(|object| {
                        object.kind() == IDENTIFIER
                            && context.text(object) == NUMBER_GLOBAL
                            && constants.is_global(object)
                    })
        }
        _ => false,
    }
}
