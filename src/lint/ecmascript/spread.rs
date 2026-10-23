use tree_sitter::Node;

use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, INDEX_FIELD, MEMBER, OBJECT_FIELD,
    PROPERTY_FIELD, SUBSCRIPT, operator, without_parentheses,
};
use super::strings::static_string;
use super::tokens::equal_tokens;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;
use crate::syntax::named_children;

const APPLY: &str = "apply";
const PROPERTY_IDENTIFIER: &str = "property_identifier";
const NULL: &str = "null";
const UNDEFINED: &str = "undefined";
const VOID: &str = "void";
const UNARY: &str = "unary_expression";
const NOT_VARIADIC: &[&str] = &["array", "spread_element"];

// `prefer-spread`: `f.apply(this, args)` con el mismo `this` que tendría `f(...args)`.
pub fn spread_calls(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[CALL])
        .filter(|call| is_variadic_apply(*call, context))
        .map(|call| report(EslintRule::PreferSpread, call, APPLY))
        .collect()
}

fn is_variadic_apply(call: Node<'_>, context: &Context<'_, '_>) -> bool {
    let Some(callee) = call
        .child_by_field_name(FUNCTION_FIELD)
        .map(without_parentheses)
        .filter(|callee| is_apply_access(*callee, context))
    else {
        return false;
    };
    let Some(arguments) = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|arguments| arguments.kind() == ARGUMENTS)
    else {
        return false;
    };
    let values: Vec<Node<'_>> = named_children(arguments)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let [this_argument, spread] = values.as_slice() else {
        return false;
    };
    if NOT_VARIADIC.contains(&without_parentheses(*spread).kind()) {
        return false;
    }
    let expected_this = callee
        .child_by_field_name(OBJECT_FIELD)
        .map(without_parentheses)
        .filter(|applied| [MEMBER, SUBSCRIPT].contains(&applied.kind()))
        .and_then(|applied| applied.child_by_field_name(OBJECT_FIELD));
    expected_this.map_or_else(
        || is_null_or_undefined(*this_argument, context),
        |expected| equal_tokens(expected, *this_argument, context.source),
    )
}

fn is_apply_access(callee: Node<'_>, context: &Context<'_, '_>) -> bool {
    match callee.kind() {
        MEMBER => callee
            .child_by_field_name(PROPERTY_FIELD)
            .is_some_and(|property| {
                property.kind() == PROPERTY_IDENTIFIER && context.text(property) == APPLY
            }),
        SUBSCRIPT => callee
            .child_by_field_name(INDEX_FIELD)
            .and_then(|index| static_string(without_parentheses(index), context.source))
            .is_some_and(|name| name == APPLY),
        _ => false,
    }
}

fn is_null_or_undefined(node: Node<'_>, context: &Context<'_, '_>) -> bool {
    let node = without_parentheses(node);
    match node.kind() {
        NULL | UNDEFINED => true,
        IDENTIFIER => context.text(node) == UNDEFINED,
        UNARY => operator(node, context.source) == VOID,
        _ => false,
    }
}
