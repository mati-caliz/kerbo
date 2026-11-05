use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::modifier;
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, BINARY, BODY_FIELD, COMPOUND, DO_GROUP, ELIF_CLAUSE, ELSE_CLAUSE, EXPANSION,
    FUNCTION, IF, LIST, NEGATED, OPERATOR_FIELD, PIPELINE, POSTFIX, REDIRECTED, SUBSHELL, TEST, UNARY, WHILE,
    ancestors, descendants, is_arithmetic_command, named_children, text,
};
use crate::shell::words::braced_content;

const TEST_COMMAND: &str = "test";
const INCREMENTS: &[&str] = &["++", "--"];
const ARITHMETIC_ASSIGNMENTS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", "&=", "^=", "|=",
];
const ASSIGNING_MODIFIERS: &[&str] = &["=", ":="];

// `checkSubshelledTests` de ShellCheck: SC2233, SC2234 y SC2235.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != SUBSHELL {
        return;
    }
    let source = context.source;
    let statements = named_children(node);
    if !statements
        .iter()
        .all(|statement| is_test_structure(*statement, source))
        || has_assignment(node, source)
    {
        return;
    }
    let rule = if is_compound_condition(node) {
        ShellRule::Sc2233
    } else if is_single_test(&statements, source) && !is_function_body(node) {
        ShellRule::Sc2234
    } else {
        ShellRule::Sc2235
    };
    findings.report(rule, node, text(node, source));
}

fn unwrapped(statement: Node<'_>) -> Node<'_> {
    if statement.kind() == REDIRECTED {
        statement.child_by_field_name(BODY_FIELD).unwrap_or(statement)
    } else {
        statement
    }
}

fn is_test_structure(statement: Node<'_>, source: &[u8]) -> bool {
    let inner = unwrapped(statement);
    match inner.kind() {
        NEGATED | LIST | SUBSHELL => named_children(inner)
            .into_iter()
            .all(|child| is_test_structure(child, source)),
        COMPOUND if !is_arithmetic_command(inner, source) => named_children(inner)
            .into_iter()
            .all(|child| is_test_structure(child, source)),
        _ => is_test_command(inner, source),
    }
}

fn is_test_command(statement: Node<'_>, source: &[u8]) -> bool {
    let inner = unwrapped(statement);
    inner.kind() == TEST || Command::of(inner).is_some_and(|command| command.is_exactly(TEST_COMMAND, source))
}

fn is_single_test(statements: &[Node<'_>], source: &[u8]) -> bool {
    matches!(statements, [only] if is_test_command(*only, source))
}

fn is_function_body(subshell: Node<'_>) -> bool {
    subshell.parent().is_some_and(|parent| parent.kind() == FUNCTION)
}

fn is_compound_condition(subshell: Node<'_>) -> bool {
    let first_relevant = ancestors(subshell).find(|ancestor| ancestor.kind() != PIPELINE);
    first_relevant.is_some_and(|parent| match parent.kind() {
        IF | WHILE | ELIF_CLAUSE | ELSE_CLAUSE => true,
        DO_GROUP => parent.parent().is_some_and(|owner| owner.kind() == WHILE),
        _ => false,
    })
}

fn has_assignment(subshell: Node<'_>, source: &[u8]) -> bool {
    descendants(subshell).into_iter().any(|node| match node.kind() {
        EXPANSION => {
            let assigned = modifier(&braced_content(node, source));
            ASSIGNING_MODIFIERS
                .iter()
                .any(|operator| assigned.starts_with(operator))
        }
        BINARY if is_arithmetic(node, source) => node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| ARITHMETIC_ASSIGNMENTS.contains(&text(operator, source))),
        UNARY | POSTFIX if is_arithmetic(node, source) => node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| INCREMENTS.contains(&text(operator, source))),
        _ => false,
    })
}

fn is_arithmetic(node: Node<'_>, source: &[u8]) -> bool {
    ancestors(node)
        .any(|ancestor| ancestor.kind() == ARITHMETIC_EXPANSION || is_arithmetic_command(ancestor, source))
}
