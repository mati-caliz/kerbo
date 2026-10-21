use tree_sitter::Node;

use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const BLOCK: &str = "block";
const STATEMENT_LISTS: &[&str] = &[
    "block",
    "constructor_body",
    "switch_block_statement_group",
    "labeled_statement",
];
const BODY_FIELDS: &[&str] = &["body", "consequence", "alternative"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const IF: &str = "if_statement";
const LOOPS: &[&str] = &[
    "while_statement",
    "for_statement",
    "enhanced_for_statement",
    "do_statement",
];
const ELSE: &str = "else";
const SWITCH: &str = "switch_expression";
const SWITCH_LABEL: &str = "switch_label";
const DEFAULT: &str = "default";
const PATTERN_LABELS: &[&str] = &["type_pattern", "record_pattern", "pattern", "null_literal"];
const EMPTY_BLOCK_OWNERS: &[&str] = &[
    "while_statement",
    "for_statement",
    "enhanced_for_statement",
    "do_statement",
    "if_statement",
    "try_statement",
    "try_with_resources_statement",
    "finally_clause",
    "synchronized_statement",
    "static_initializer",
    "class_body",
];
const SWITCH_BLOCK: &str = "switch_block";
const ASSIGNMENT: &str = "assignment_expression";
const OPERATOR_FIELD: &str = "operator";
const CONDITION_FIELD: &str = "condition";
const INIT_FIELD: &str = "init";
const UPDATE_FIELD: &str = "update";
const PARENTHESIZED: &str = "parenthesized_expression";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const TERNARY: &str = "ternary_expression";
const ALLOWED_ASSIGNMENT_PARENTS: &[&str] = &["expression_statement", "lambda_expression", "resource"];
const COMPARISONS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];
const CONDITION_LOOPS: &[&str] = &["while_statement", "do_statement", "for_statement"];
const BOOLEAN_OPERATORS: &[&str] = &["==", "!=", "&&", "||"];
const BOOLEAN_LITERALS: &[&str] = &["true", "false"];
const NEGATION: &str = "!";
const QUESTION_MARK: &str = "?";
const LOGICAL_OPERATORS: &[&str] = &["&&", "||"];

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for node in descendants(root) {
        let kind = node.kind();
        if kind == IF || LOOPS.contains(&kind) {
            findings.extend(missing_braces(node));
        }
        if kind == SWITCH && is_statement(node) && !has_default(node) {
            findings.push(finding(
                LintKind::Java(JavaRule::MissingSwitchDefault),
                node,
                "",
                None,
            ));
        }
        if kind == BLOCK || kind == SWITCH_BLOCK {
            findings.extend(block_findings(node));
        }
        if kind == ASSIGNMENT && !is_allowed_assignment(node) {
            let operator = node.child_by_field_name(OPERATOR_FIELD).unwrap_or(node);
            findings.push(finding(
                LintKind::Java(JavaRule::InnerAssignment),
                operator,
                "",
                None,
            ));
        }
        findings.extend(simplifiable(node, source));
    }
    findings
}

fn is_statement(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        STATEMENT_LISTS.contains(&parent.kind())
            || BODY_FIELDS
                .iter()
                .any(|field| parent.child_by_field_name(field) == Some(node))
    })
}

// Un `else if` no pide llaves: la cadena sigue.
fn missing_braces(statement: Node<'_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let body_field = if statement.kind() == IF {
        "consequence"
    } else {
        "body"
    };
    if statement
        .child_by_field_name(body_field)
        .is_some_and(|body| body.kind() != BLOCK)
    {
        findings.push(finding(LintKind::Java(JavaRule::NeedBraces), statement, "", None));
    }
    if statement.kind() == IF
        && let Some(alternative) = statement.child_by_field_name("alternative")
        && alternative.kind() != BLOCK
        && alternative.kind() != IF
        && let Some(keyword) = super::java::children(statement)
            .into_iter()
            .find(|child| child.kind() == ELSE)
    {
        findings.push(finding(LintKind::Java(JavaRule::NeedBraces), keyword, "", None));
    }
    findings
}

// Checkstyle no exige `default` en un switch con patrones o con `case null`: ahí el compilador ya
// pide que cubra todo.
fn has_default(switch: Node<'_>) -> bool {
    let cases = switch
        .child_by_field_name("body")
        .map(named_children)
        .unwrap_or_default();
    cases.into_iter().flat_map(named_children).any(|label| {
        label.kind() == SWITCH_LABEL
            && super::java::children(label)
                .into_iter()
                .any(|child| child.kind() == DEFAULT || PATTERN_LABELS.contains(&child.kind()))
    })
}

fn block_findings(block: Node<'_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let Some(parent) = block.parent() else {
        return findings;
    };
    if block.kind() == BLOCK
        && STATEMENT_LISTS.contains(&parent.kind())
        && parent.kind() != "labeled_statement"
    {
        findings.push(finding(
            LintKind::Java(JavaRule::AvoidNestedBlocks),
            block,
            "",
            None,
        ));
    }
    let owned = (block.kind() == SWITCH_BLOCK && parent.kind() == SWITCH && is_statement(parent))
        || (block.kind() == BLOCK && EMPTY_BLOCK_OWNERS.contains(&parent.kind()));
    let empty = named_children(block)
        .iter()
        .all(|child| COMMENTS.contains(&child.kind()));
    if owned && empty {
        findings.push(finding(LintKind::Java(JavaRule::EmptyBlock), block, "", None));
    }
    findings
}

// Una asignación vale como sentencia, en el inicio o el avance de un `for`, como cuerpo de una
// lambda o en un recurso, y en el idioma `while ((linea = leer()) != null)`.
fn is_allowed_assignment(assignment: Node<'_>) -> bool {
    let Some(parent) = assignment.parent() else {
        return true;
    };
    if ALLOWED_ASSIGNMENT_PARENTS.contains(&parent.kind()) {
        return true;
    }
    let in_for_header = parent.kind() == "for_statement"
        && [INIT_FIELD, UPDATE_FIELD].iter().any(|field| {
            let mut cursor = parent.walk();
            parent
                .children_by_field_name(field, &mut cursor)
                .any(|child| child == assignment)
        });
    in_for_header || is_in_loop_condition_idiom(assignment)
}

fn is_in_loop_condition_idiom(assignment: Node<'_>) -> bool {
    let Some(comparison) = skip_parentheses_up(assignment).filter(|node| {
        node.kind() == BINARY
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| COMPARISONS.contains(&operator.kind()))
    }) else {
        return false;
    };
    let mut owner = comparison.parent();
    while let Some(node) = owner.filter(|node| node.kind() == PARENTHESIZED || is_logical(*node)) {
        owner = node.parent();
    }
    owner.is_some_and(|loop_statement| {
        CONDITION_LOOPS.contains(&loop_statement.kind())
            && loop_statement
                .child_by_field_name(CONDITION_FIELD)
                .is_some_and(|condition| condition.byte_range().contains(&comparison.start_byte()))
    })
}

fn is_logical(node: Node<'_>) -> bool {
    node.kind() == BINARY
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| LOGICAL_OPERATORS.contains(&operator.kind()))
}

fn skip_parentheses_up(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent()?;
    while current.kind() == PARENTHESIZED {
        current = current.parent()?;
    }
    Some(current)
}

fn simplifiable(node: Node<'_>, source: &[u8]) -> Option<LintFinding> {
    let is_literal = |field: &str| {
        node.child_by_field_name(field)
            .is_some_and(|operand| BOOLEAN_LITERALS.contains(&operand.kind()))
    };
    let operator = node.child_by_field_name(OPERATOR_FIELD);
    let operator_text = operator.and_then(|operator| operator.utf8_text(source).ok());
    let simplifiable = match node.kind() {
        BINARY => {
            operator_text.is_some_and(|text| BOOLEAN_OPERATORS.contains(&text))
                && (is_literal("left") || is_literal("right"))
        }
        UNARY => operator_text == Some(NEGATION) && is_literal("operand"),
        TERNARY => is_literal("consequence") && is_literal("alternative"),
        _ => false,
    };
    if !simplifiable {
        return None;
    }
    let position = if node.kind() == TERNARY {
        super::java::children(node)
            .into_iter()
            .find(|child| child.kind() == QUESTION_MARK)
    } else {
        operator
    }?;
    Some(finding(
        LintKind::Java(JavaRule::SimplifyBooleanExpression),
        position,
        "",
        None,
    ))
}
