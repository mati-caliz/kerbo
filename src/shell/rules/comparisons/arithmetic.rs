use tree_sitter::Node;

use super::test_operators::is_literal_number;
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ASSIGNMENT, BINARY, BODY_FIELD, COMMAND, CONCATENATION,
    DESTINATION_FIELD, FILE_REDIRECT, HEREDOC_BODY, HEREDOC_REDIRECT, HERESTRING, INDEX_FIELD, LEFT_FIELD,
    NAME_FIELD, NUMBER, OPERATOR_FIELD, PARENTHESIZED, POSTFIX, REDIRECT_FIELD, REDIRECTED, RIGHT_FIELD,
    STRING, SUBSCRIPT, SUBSHELL, TERNARY, UNARY, WORD, ancestors, field_children, is_arithmetic_command,
    named_children, text,
};

const LOGICAL_OR: &str = "||";
const LOGICAL_AND: &str = "&&";
const NOT_EQUAL: &str = "!=";
const EQUAL: &str = "==";
const MULTIPLY: &str = "*";
const DIVIDE: &str = "/";
const INCREMENT: &str = "++";
const DECREMENT: &str = "--";
const ASSIGNMENT_OPERATORS: &[&str] = &["=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|="];
const FOR_CLAUSES: &[&str] = &["initializer", "condition", "update"];
const ARITHMETIC_PARTS: &[&str] = &[BINARY, UNARY, POSTFIX, PARENTHESIZED, TERNARY];
const OCTAL_START: char = '0';

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    match node.kind() {
        BINARY if is_in_arithmetic(node, source) => check_binary(node, context, findings),
        NUMBER | WORD if is_in_arithmetic(node, source) => check_octal(node, source, findings),
        PARENTHESIZED if is_in_arithmetic(node, source) => check_double_parentheses(node, source, findings),
        ARITHMETIC_EXPANSION => check_leading_parenthesis(&named_children(node), source, findings),
        ARITHMETIC_FOR => {
            for clause in FOR_CLAUSES {
                check_leading_parenthesis(&field_children(node, clause), source, findings);
            }
        }
        ASSIGNMENT => check_index_parenthesis(node, source, findings),
        REDIRECTED | COMMAND => check_modified_in_redirect(node, context, findings),
        _ => {
            if is_arithmetic_command(node, source) {
                check_leading_parenthesis(&named_children(node), source, findings);
            }
        }
    }
}

fn is_in_arithmetic(node: Node<'_>, source: &[u8]) -> bool {
    for ancestor in ancestors(node) {
        if [ARITHMETIC_EXPANSION, ARITHMETIC_FOR].contains(&ancestor.kind())
            || is_arithmetic_command(ancestor, source)
        {
            return true;
        }
        if !ARITHMETIC_PARTS.contains(&ancestor.kind()) {
            return false;
        }
    }
    false
}

fn operator_text<'source>(expression: Node<'_>, source: &'source [u8]) -> &'source str {
    expression
        .child_by_field_name(OPERATOR_FIELD)
        .map_or("", |operator| text(operator, source))
}

struct BinaryParts<'tree, 'source> {
    operator: &'source str,
    left: Node<'tree>,
    right: Node<'tree>,
}

fn binary_parts<'tree, 'source>(
    expression: Node<'tree>,
    source: &'source [u8],
) -> Option<BinaryParts<'tree, 'source>> {
    (expression.kind() == BINARY).then_some(())?;
    Some(BinaryParts {
        operator: operator_text(expression, source),
        left: expression.child_by_field_name(LEFT_FIELD)?,
        right: expression.child_by_field_name(RIGHT_FIELD)?,
    })
}

// SC2056, SC2334 y SC2017: `checkOrNeq`, `checkAndEq` y `checkDivBeforeMult`.
fn check_binary(expression: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let Some(parts) = binary_parts(expression, source) else {
        return;
    };
    let Some(operator) = expression.child_by_field_name(OPERATOR_FIELD) else {
        return;
    };
    if let (Some(left), Some(right)) = (
        binary_parts(parts.left, source),
        binary_parts(parts.right, source),
    ) {
        let same_subject = text(left.left, source) == text(right.left, source);
        if parts.operator == LOGICAL_OR
            && left.operator == NOT_EQUAL
            && right.operator == NOT_EQUAL
            && same_subject
        {
            findings.report(ShellRule::Sc2056, operator, LOGICAL_OR);
        }
        if parts.operator == LOGICAL_AND
            && left.operator == EQUAL
            && right.operator == EQUAL
            && same_subject
            && is_literal_number(left.right, source)
            && is_literal_number(right.right, source)
        {
            findings.report(ShellRule::Sc2334, operator, LOGICAL_AND);
        }
    }
    if parts.operator == MULTIPLY
        && context.dialect != Dialect::Ksh
        && let Some(division) = binary_parts(parts.left, source)
        && division.operator == DIVIDE
        && text(division.right, source) != text(parts.right, source)
        && let Some(divide) = parts.left.child_by_field_name(OPERATOR_FIELD)
    {
        findings.report(ShellRule::Sc2017, divide, DIVIDE);
    }
}

// SC2080: `checkArithmeticBadOctal`, la expresión `^0[0-7]*[8-9]`.
fn check_octal(number: Node<'_>, source: &[u8], findings: &mut Findings) {
    let raw = text(number, source);
    let Some(rest) = raw.strip_prefix(OCTAL_START) else {
        return;
    };
    let first_not_octal = rest
        .chars()
        .find(|digit| !digit.is_ascii_digit() || matches!(digit, '8' | '9'));
    if matches!(first_not_octal, Some('8' | '9')) {
        findings.report(ShellRule::Sc2080, number, raw);
    }
}

// SC2322: `checkUnnecessaryParens`, `((x))` dentro de una expresión.
fn check_double_parentheses(outer: Node<'_>, source: &[u8], findings: &mut Findings) {
    if let [only] = named_children(outer)[..]
        && only.kind() == PARENTHESIZED
    {
        findings.report(ShellRule::Sc2322, only, text(only, source));
    }
}

// SC2323: `checkUnnecessaryParens`, paréntesis que envuelven toda la expresión.
fn check_leading_parenthesis(contents: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    if let [only] = contents
        && only.kind() == PARENTHESIZED
    {
        findings.report(ShellRule::Sc2323, *only, text(*only, source));
    }
}

fn check_index_parenthesis(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(index) = assignment
        .child_by_field_name(NAME_FIELD)
        .filter(|name| name.kind() == SUBSCRIPT)
        .and_then(|name| name.child_by_field_name(INDEX_FIELD))
    else {
        return;
    };
    if [SUBSHELL, PARENTHESIZED].contains(&index.kind()) {
        findings.report(ShellRule::Sc2323, index, text(index, source));
    }
}

// SC2257: `checkModifiedArithmeticInRedirection`.
fn check_modified_in_redirect(statement: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if matches!(context.dialect, Dialect::Dash | Dialect::Busybox) {
        return;
    }
    let body = if statement.kind() == REDIRECTED {
        statement.child_by_field_name(BODY_FIELD)
    } else {
        Some(statement)
    };
    let has_words = body
        .and_then(Command::of)
        .is_some_and(|command| !command.words.is_empty());
    if !has_words {
        return;
    }
    for redirect in field_children(statement, REDIRECT_FIELD) {
        check_redirect_targets(redirect, context.source, findings);
    }
}

fn check_redirect_targets(redirect: Node<'_>, source: &[u8], findings: &mut Findings) {
    let targets = match redirect.kind() {
        FILE_REDIRECT => field_children(redirect, DESTINATION_FIELD),
        HERESTRING => named_children(redirect),
        HEREDOC_REDIRECT => named_children(redirect)
            .into_iter()
            .filter(|child| child.kind() == HEREDOC_BODY)
            .flat_map(named_children)
            .collect(),
        _ => Vec::new(),
    };
    let expressions = targets
        .into_iter()
        .flat_map(word_parts)
        .filter(|part| part.kind() == ARITHMETIC_EXPANSION)
        .flat_map(named_children);
    for expression in expressions {
        report_modifying(expression, source, findings);
    }
}

fn word_parts(word: Node<'_>) -> Vec<Node<'_>> {
    if [CONCATENATION, STRING].contains(&word.kind()) {
        return named_children(word).into_iter().flat_map(word_parts).collect();
    }
    vec![word]
}

fn report_modifying(expression: Node<'_>, source: &[u8], findings: &mut Findings) {
    match expression.kind() {
        UNARY | POSTFIX => {
            let Some(operator) = expression.child_by_field_name(OPERATOR_FIELD) else {
                return;
            };
            let operator_text = text(operator, source);
            if [INCREMENT, DECREMENT].contains(&operator_text) {
                findings.report(ShellRule::Sc2257, operator, operator_text);
            }
        }
        BINARY => {
            let Some(parts) = binary_parts(expression, source) else {
                return;
            };
            if ASSIGNMENT_OPERATORS.contains(&parts.operator) {
                if let Some(operator) = expression.child_by_field_name(OPERATOR_FIELD) {
                    findings.report(ShellRule::Sc2257, operator, parts.operator);
                }
            } else {
                report_modifying(parts.left, source, findings);
                report_modifying(parts.right, source, findings);
            }
        }
        TERNARY => {
            for part in named_children(expression) {
                report_modifying(part, source, findings);
            }
        }
        _ => {}
    }
}
