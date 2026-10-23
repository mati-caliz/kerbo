use tree_sitter::Node;

use super::constants::{BOOLEAN, Constants, NOT, UNARY};
use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, CALL, CONDITION_FIELD, FUNCTION_FIELD, IDENTIFIER, IF, first_named_child,
    operator, parent_without_parentheses, without_parentheses,
};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const TESTED_STATEMENTS: &[&str] = &[
    IF,
    "do_statement",
    "while_statement",
    "ternary_expression",
    "for_statement",
];
const EMPTY_STATEMENT: &str = "empty_statement";

// `no-extra-boolean-cast`: `!!x` o `Boolean(x)` donde el valor ya se usa como booleano.
pub fn extra_boolean_casts(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    let negations = context
        .of_kind(&[UNARY])
        .filter(|inner| operator(*inner, context.source) == NOT)
        .filter_map(|inner| parent_without_parentheses(inner))
        .filter(|outer| outer.kind() == UNARY && operator(*outer, context.source) == NOT);
    let calls = context
        .of_kind(&[CALL])
        .filter(|call| {
            call.child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| arguments.kind() == ARGUMENTS)
        })
        .filter(|call| is_boolean_callee(call.child_by_field_name(FUNCTION_FIELD), &constants, context));
    negations
        .chain(calls)
        .filter(|cast| is_in_boolean_context(*cast, &constants, context))
        .map(|cast| report(EslintRule::NoExtraBooleanCast, cast, ""))
        .collect()
}

fn is_boolean_callee(
    callee: Option<Node<'_>>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    callee.map(without_parentheses).is_some_and(|callee| {
        callee.kind() == IDENTIFIER && context.text(callee) == BOOLEAN && constants.is_global(callee)
    })
}

fn is_in_boolean_context(
    node: Node<'_>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    let Some(parent) = parent_without_parentheses(node) else {
        return false;
    };
    if parent.kind() == UNARY {
        return operator(parent, context.source) == NOT;
    }
    if parent.kind() == ARGUMENTS {
        let is_first = first_named_child(parent)
            .map(without_parentheses)
            .is_some_and(|first| first.id() == node.id());
        return is_first
            && parent.parent().is_some_and(|call| {
                let callee = match call.kind() {
                    CALL => call.child_by_field_name(FUNCTION_FIELD),
                    NEW => call.child_by_field_name(CONSTRUCTOR_FIELD),
                    _ => None,
                };
                is_boolean_callee(callee, constants, context)
            });
    }
    TESTED_STATEMENTS.contains(&parent.kind()) && test_of(parent).is_some_and(|test| test.id() == node.id())
}

fn test_of(statement: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = statement.walk();
    let test = statement
        .children_by_field_name(CONDITION_FIELD, &mut cursor)
        .find(|condition| condition.is_named() && condition.kind() != EMPTY_STATEMENT);
    test.map(without_parentheses)
}
