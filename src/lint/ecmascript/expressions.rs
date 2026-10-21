use tree_sitter::Node;

use super::nodes::{COMMENT, STRING, TEMPLATE_STRING, first_named_child, is_function, without_parentheses};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const STATEMENT: &str = "expression_statement";
const ALWAYS_UNUSED: &[&str] = &[
    "array",
    "arrow_function",
    "binary_expression",
    "class",
    "ternary_expression",
    "function_expression",
    "function",
    "generator_function",
    "identifier",
    "undefined",
    STRING,
    "number",
    "regex",
    "true",
    "false",
    "null",
    TEMPLATE_STRING,
    "member_expression",
    "subscript_expression",
    "meta_property",
    "object",
    "sequence_expression",
    "this",
];
const UNWRAPPED: &[&str] = &[
    "as_expression",
    "type_assertion",
    "non_null_expression",
    "instantiation_expression",
];
const KEPT_UNARY: &[&str] = &["void", "delete"];
const PROGRAM: &str = "program";
const MEMBER: &str = "member_expression";
const SATISFIES: &str = "satisfies_expression";
const HASH_BANG: &str = "hash_bang_line";
const MODULE_HOSTS: &[&str] = &["internal_module", "module"];

pub fn unused_expressions(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[STATEMENT])
        .filter(|statement| {
            first_named_child(*statement).is_some_and(|expression| is_disallowed(expression, context))
        })
        .filter(|statement| !is_directive(*statement))
        .map(|statement| report(EslintRule::NoUnusedExpressions, statement, ""))
        .collect()
}

fn is_disallowed(expression: Node<'_>, context: &Context<'_, '_>) -> bool {
    let expression = without_parentheses(expression);
    let kind = expression.kind();
    if kind == MEMBER && is_qualified_satisfies(expression) {
        return false;
    }
    if ALWAYS_UNUSED.contains(&kind) {
        return true;
    }
    if UNWRAPPED.contains(&kind) {
        return wrapped_expression(expression).is_some_and(|inner| is_disallowed(inner, context));
    }
    match kind {
        "unary_expression" => expression
            .child_by_field_name("operator")
            .is_some_and(|operator| !KEPT_UNARY.contains(&context.text(operator))),
        "call_expression" => expression
            .child_by_field_name("arguments")
            .is_some_and(|arguments| arguments.kind() == TEMPLATE_STRING),
        _ => false,
    }
}

fn is_qualified_satisfies(member: Node<'_>) -> bool {
    let mut object = member.child_by_field_name("object");
    while let Some(current) = object.filter(|current| current.kind() == MEMBER) {
        object = current.child_by_field_name("object");
    }
    object.is_some_and(|object| object.kind() == SATISFIES)
}

fn wrapped_expression(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "type_assertion" {
        let mut cursor = node.walk();
        let inner = node
            .named_children(&mut cursor)
            .find(|child| child.kind() != "type_arguments" && child.kind() != COMMENT);
        return inner;
    }
    node.child_by_field_name("function")
        .or_else(|| first_named_child(node))
}

fn is_directive(statement: Node<'_>) -> bool {
    let Some(parent) = statement.parent() else {
        return false;
    };
    let is_prologue_host = parent.kind() == PROGRAM
        || (parent.kind() == "statement_block"
            && parent
                .parent()
                .is_some_and(|host| is_function(host) || MODULE_HOSTS.contains(&host.kind())));
    if !is_prologue_host {
        return false;
    }
    let mut cursor = parent.walk();
    for sibling in parent
        .named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT && child.kind() != HASH_BANG)
    {
        let is_string_statement = sibling.kind() == STATEMENT
            && first_named_child(sibling).is_some_and(|expression| expression.kind() == STRING);
        if !is_string_statement {
            return false;
        }
        if sibling.id() == statement.id() {
            return true;
        }
    }
    false
}
