use std::collections::HashSet;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

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

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let mut allowed_assignments: HashSet<usize> = HashSet::new();
    for &node in nodes {
        allowed_assignments.extend(allowed_inside(node).into_iter().map(|assignment| assignment.id()));
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
        findings.extend(block_findings(node));
        if kind == ASSIGNMENT && !allowed_assignments.contains(&node.id()) {
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

// Los bloques se miran desde su dueño: un bloque suelto en una lista de sentencias, y uno vacío como
// cuerpo de un if, un bucle, un try, un finally, un bloque static o de inicialización, o un switch.
// Subir al padre en tree-sitter es caro.
fn block_findings(owner: Node<'_>) -> Vec<LintFinding> {
    let kind = owner.kind();
    let blocks: Vec<Node<'_>> = named_children(owner)
        .into_iter()
        .filter(|child| child.kind() == BLOCK)
        .collect();
    let mut findings = Vec::new();
    if STATEMENT_LISTS.contains(&kind) && kind != "labeled_statement" {
        findings.extend(
            blocks
                .iter()
                .map(|block| finding(LintKind::Java(JavaRule::AvoidNestedBlocks), *block, "", None)),
        );
    }
    let is_empty = |block: &Node<'_>| {
        named_children(*block)
            .iter()
            .all(|child| COMMENTS.contains(&child.kind()))
    };
    if EMPTY_BLOCK_OWNERS.contains(&kind) {
        findings.extend(
            blocks
                .iter()
                .filter(|block| is_empty(block))
                .map(|block| finding(LintKind::Java(JavaRule::EmptyBlock), *block, "", None)),
        );
    }
    if kind == SWITCH
        && is_statement(owner)
        && let Some(body) = owner
            .child_by_field_name("body")
            .filter(|body| body.kind() == SWITCH_BLOCK && is_empty(body))
    {
        findings.push(finding(LintKind::Java(JavaRule::EmptyBlock), body, "", None));
    }
    findings
}

// Una asignación vale como sentencia, en el inicio o el avance de un `for`, como cuerpo de una
// lambda o en un recurso, y en el idioma `while ((linea = leer()) != null)`, también combinado con
// `&&` o `||`. Se marcan desde el contenedor, que el recorrido visita antes que la asignación.
fn allowed_inside(container: Node<'_>) -> Vec<Node<'_>> {
    let kind = container.kind();
    if ALLOWED_ASSIGNMENT_PARENTS.contains(&kind) {
        return named_children(container)
            .into_iter()
            .filter(|child| child.kind() == ASSIGNMENT)
            .collect();
    }
    let mut allowed = Vec::new();
    if kind == "for_statement" {
        for field in [INIT_FIELD, UPDATE_FIELD] {
            let mut cursor = container.walk();
            allowed.extend(
                container
                    .children_by_field_name(field, &mut cursor)
                    .filter(|child| child.kind() == ASSIGNMENT),
            );
        }
    }
    if CONDITION_LOOPS.contains(&kind)
        && let Some(condition) = container.child_by_field_name(CONDITION_FIELD)
    {
        allowed.extend(compared_assignments(condition));
    }
    allowed
}

fn compared_assignments(condition: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![condition];
    while let Some(node) = pending.pop() {
        if node.kind() == PARENTHESIZED || is_logical(node) {
            pending.extend(named_children(node));
        } else if is_comparison(node) {
            found.extend(
                ["left", "right"]
                    .iter()
                    .filter_map(|field| node.child_by_field_name(field))
                    .filter_map(skip_parentheses_down)
                    .filter(|operand| operand.kind() == ASSIGNMENT),
            );
        }
    }
    found
}

fn is_comparison(node: Node<'_>) -> bool {
    node.kind() == BINARY
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| COMPARISONS.contains(&operator.kind()))
}

fn is_logical(node: Node<'_>) -> bool {
    node.kind() == BINARY
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| LOGICAL_OPERATORS.contains(&operator.kind()))
}

fn skip_parentheses_down(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        current = named_children(current).into_iter().next()?;
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
