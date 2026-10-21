use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::quoting::is_condition;
use crate::shell::syntax::{
    AND, ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, COMPOUND, CONDITION_FIELD, EXPANSIONS, FUNCTION, IF, LIST,
    NEGATED, OPERATOR_FIELD, OR, PARENTHESIZED, PIPELINE, REDIRECTED, SUBSHELL, TEST, UNARY, ancestors,
    children, field_children, is_arithmetic_command, is_double_bracket, named_children, text,
};
use crate::shell::test_expression::{Condition, parse};
use crate::shell::words::{braced_content, literal, parts};

const SINGLE_BRACKET_AND: &str = "-a";
const SINGLE_BRACKET_OR: &str = "-o";
const EXIT_CODE: &str = "?";
const ZERO: &str = "0";
const NEGATION: &str = "!";
const NOT_SUCCESS_ON_LEFT: &[&str] = &["-gt", "-ne", "!=", "!"];
const NOT_SUCCESS_ON_RIGHT: &[&str] = &["-ne", "!="];
const ARITHMETIC_COMPARISONS: &[&str] = &[">", "<", ">=", "<=", "==", "!="];
const BINARY_KIND: &str = "binary_expression";
const SAFE_FALLBACKS: &[&str] = &["echo", "exit", "return", "printf", "true", ":"];
const TEST_COMMAND: &str = "test";
const PATH_SEPARATOR: char = '/';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() == TEST {
        check_test(node, context, findings);
    } else if is_arithmetic_command(node, context.source) {
        check_arithmetic(node, context, findings);
    } else if node.kind() == LIST {
        check_shorthand_if(node, context, findings);
    }
}

fn check_test(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(condition) = parse(node, context.source) else {
        return;
    };
    let single = !is_double_bracket(node, context.source);
    condition.visit(true, &mut |part, only_test| match part {
        Condition::And(operator, ..) | Condition::Or(operator, ..) if single => {
            let operator_text = text(*operator, context.source);
            if [SINGLE_BRACKET_AND, SINGLE_BRACKET_OR].contains(&operator_text) {
                findings.report(ShellRule::Sc2166, *operator, operator_text);
            }
        }
        Condition::Binary(operator, left, right) if only_test => {
            compare_exit_code(
                text(*operator, context.source),
                (*left, *right),
                context,
                findings,
            );
        }
        Condition::And(..)
        | Condition::Or(..)
        | Condition::Binary(..)
        | Condition::Not(_)
        | Condition::Group(_)
        | Condition::Unary
        | Condition::Nullary => {}
    });
}

fn compare_exit_code(
    operator: &str,
    (left, right): (Node<'_>, Node<'_>),
    context: &Context<'_, '_>,
    findings: &mut Findings,
) {
    let source = context.source;
    if is_zero(right, source) && is_exit_code(left, source) {
        report_exit_code(left, !NOT_SUCCESS_ON_LEFT.contains(&operator), context, findings);
    } else if is_zero(left, source) && is_exit_code(right, source) {
        report_exit_code(
            right,
            !NOT_SUCCESS_ON_RIGHT.contains(&operator),
            context,
            findings,
        );
    }
}

fn report_exit_code(node: Node<'_>, for_success: bool, context: &Context<'_, '_>, findings: &mut Findings) {
    if is_first_command_in_function(node, context.source) {
        return;
    }
    let shown = if for_success { "if mycmd;" } else { "if ! mycmd;" };
    findings.report(ShellRule::Sc2181, node, shown);
}

fn is_zero(word: Node<'_>, source: &[u8]) -> bool {
    literal(word, source).as_deref() == Some(ZERO)
}

fn is_exit_code(word: Node<'_>, source: &[u8]) -> bool {
    matches!(parts(word).as_slice(), [only] if EXPANSIONS.contains(&only.kind()) && braced_content(*only, source) == EXIT_CODE)
}

fn check_arithmetic(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    for expression in arithmetic_descendants(node) {
        if expression.kind() == BINARY_KIND {
            let operator = expression
                .child_by_field_name(OPERATOR_FIELD)
                .map_or("", |operator| text(operator, source));
            let left = expression.child_by_field_name(crate::shell::syntax::LEFT_FIELD);
            let right = expression.child_by_field_name(crate::shell::syntax::RIGHT_FIELD);
            if let (Some(left), Some(right)) = (left, right)
                && ARITHMETIC_COMPARISONS.contains(&operator)
                && is_only_arithmetic_test(expression, source)
            {
                compare_exit_code(operator, (left, right), context, findings);
            }
        } else if expression.kind() == UNARY
            && unary_operator(expression, source) == NEGATION
            && let Some(operand) = named_children(expression).last().copied()
            && is_exit_code(operand, source)
            && is_only_arithmetic_test(expression, source)
        {
            report_exit_code(operand, false, context, findings);
        } else if is_sole_content(expression, source) && is_exit_code(expression, source) {
            let holder = expression.parent();
            if holder.is_some_and(|holder| {
                is_arithmetic_command(holder, source) || is_only_arithmetic_test(holder, source)
            }) {
                report_exit_code(expression, false, context, findings);
            }
        }
    }
}

fn arithmetic_descendants(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = named_children(node);
    while let Some(current) = pending.pop() {
        if EXPANSIONS.contains(&current.kind()) || ARITHMETIC_PARTS.contains(&current.kind()) {
            found.push(current);
        }
        if ARITHMETIC_PARTS.contains(&current.kind()) {
            pending.extend(named_children(current));
        }
    }
    found
}

const ARITHMETIC_PARTS: &[&str] = &[
    BINARY_KIND,
    UNARY,
    PARENTHESIZED,
    "postfix_expression",
    "ternary_expression",
];

fn unary_operator<'source>(expression: Node<'_>, source: &'source [u8]) -> &'source str {
    expression
        .child_by_field_name(OPERATOR_FIELD)
        .map_or("", |operator| text(operator, source))
}

fn is_sole_content(expression: Node<'_>, source: &[u8]) -> bool {
    expression.parent().is_some_and(|parent| {
        (is_arithmetic_command(parent, source) || parent.kind() == PARENTHESIZED)
            && named_children(parent).len() == 1
    })
}

fn is_only_arithmetic_test(expression: Node<'_>, source: &[u8]) -> bool {
    let Some(parent) = expression.parent() else {
        return false;
    };
    if is_arithmetic_command(parent, source) {
        return named_children(parent).len() == 1;
    }
    let passes = (parent.kind() == UNARY && unary_operator(parent, source) == NEGATION)
        || (parent.kind() == PARENTHESIZED && named_children(parent).len() == 1);
    passes && is_only_arithmetic_test(parent, source)
}

fn is_first_command_in_function(token: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = ancestors(token).find(|ancestor| ancestor.kind() == FUNCTION) else {
        return false;
    };
    let closest = ancestors(token).find(|ancestor| is_command_like(*ancestor, source));
    let first = first_command(function, source);
    closest.map(unwrap_redirected) == Some(unwrap_redirected(first))
}

fn is_command_like(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == TEST || is_arithmetic_command(node, source) || Command::of(node).is_some()
}

fn unwrap_redirected(node: Node<'_>) -> Node<'_> {
    if node.kind() == REDIRECTED {
        node.child_by_field_name(BODY_FIELD).unwrap_or(node)
    } else {
        node
    }
}

fn first_command<'tree>(node: Node<'tree>, source: &[u8]) -> Node<'tree> {
    let inner = match node.kind() {
        FUNCTION => node.child_by_field_name(BODY_FIELD),
        SUBSHELL | LIST | PIPELINE => named_children(node).first().copied(),
        COMPOUND if !is_arithmetic_command(node, source) => named_children(node).first().copied(),
        IF => field_children(node, CONDITION_FIELD)
            .into_iter()
            .find(Node::is_named),
        REDIRECTED => node
            .child_by_field_name(BODY_FIELD)
            .filter(|body| body.kind() == IF),
        _ => None,
    };
    inner.map_or(node, |inner| first_command(inner, source))
}

fn check_shorthand_if(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let [left, right] = named_children(node)[..] else {
        return;
    };
    let left = unwrap_redirected(left);
    if operator_of(node, source) != Some(OR) || left.kind() != LIST || operator_of(left, source) != Some(AND)
    {
        return;
    }
    let Some(tested) = named_children(left).last().copied() else {
        return;
    };
    if right.kind() == LIST || right.kind() == NEGATED || is_test_command(tested, source) {
        return;
    }
    if is_harmless_fallback(right, source) || is_condition(node, source) {
        return;
    }
    if let Some(operator) = children(left)
        .into_iter()
        .find(|child| text(*child, source) == AND)
    {
        findings.report(ShellRule::Sc2015, operator, "");
    }
}

fn operator_of<'source>(list: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    children(list)
        .into_iter()
        .find(|child| !child.is_named())
        .map(|operator| text(operator, source))
}

fn is_test_command(node: Node<'_>, source: &[u8]) -> bool {
    let node = unwrap_redirected(node);
    node.kind() == TEST
        || Command::of(node).is_some_and(|command| {
            command.name(source).is_some_and(|name| {
                name == TEST_COMMAND
                    || name
                        .strip_suffix(TEST_COMMAND)
                        .is_some_and(|prefix| prefix.ends_with(PATH_SEPARATOR))
            })
        })
}

fn is_harmless_fallback(node: Node<'_>, source: &[u8]) -> bool {
    let node = unwrap_redirected(node);
    if [ASSIGNMENT, ASSIGNMENTS].contains(&node.kind()) {
        return true;
    }
    Command::of(node).is_some_and(|command| {
        let only_assignments = command.words.is_empty() && !command.assignments.is_empty();
        only_assignments
            || command.name(source).is_some_and(|name| {
                let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or(&name);
                SAFE_FALLBACKS.contains(&basename)
            })
    })
}
