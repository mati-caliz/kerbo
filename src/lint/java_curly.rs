use tree_sitter::{Node, Point};

use super::java::children;
use super::java_tokens::Lines;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const OPEN_CURLY: &str = "{";
const CLOSE_CURLY: &str = "}";
const BLOCK: &str = "block";
const CONSTRUCTOR_BODY: &str = "constructor_body";
const TYPE_BODIES: &[&str] = &[
    "class_body",
    "interface_body",
    "enum_body",
    "annotation_type_body",
    "switch_block",
];
const BODY_FIELD: &str = "body";
const BODY_OWNERS: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "compact_constructor_declaration",
    "for_statement",
    "enhanced_for_statement",
    "while_statement",
    "do_statement",
    "try_statement",
    "try_with_resources_statement",
    "catch_clause",
    "synchronized_statement",
    "lambda_expression",
];
const BLOCK_CHILD_OWNERS: &[&str] = &["finally_clause", "static_initializer", "switch_rule"];
const IF: &str = "if_statement";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const ELSE: &str = "else";
const SWITCH_GROUP: &str = "switch_block_statement_group";
const SWITCH_LABEL: &str = "switch_label";
const SWITCH_BLOCK: &str = "switch_block";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const CATCH: &str = "catch_clause";
const FINALLY: &str = "finally_clause";
const NESTING_STATEMENTS: &[&str] = &[
    "for_statement",
    "enhanced_for_statement",
    "while_statement",
    "labeled_statement",
];
const SEMICOLON_ENDED: &[&str] = &["expression_statement", "local_variable_declaration"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const SEMICOLON: &str = ";";

pub fn reported(nodes: &[Node<'_>], lines: &Lines<'_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for &node in nodes {
        findings.extend(left_curlies(node, lines));
        match node.kind() {
            BLOCK | CONSTRUCTOR_BODY => {
                let closing = last_child(node).filter(|child| child.kind() == CLOSE_CURLY);
                findings.extend(right_curlies_in(&statements(node), closing.map(Next::of), lines));
            }
            SWITCH_BLOCK => findings.extend(switch_groups(node, lines)),
            _ => {}
        }
    }
    findings
}

fn statements(container: Node<'_>) -> Vec<Node<'_>> {
    named_children(container)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()) && child.kind() != SWITCH_LABEL)
        .collect()
}

fn last_child(node: Node<'_>) -> Option<Node<'_>> {
    children(node).into_iter().last()
}

fn first_child(node: Node<'_>) -> Option<Node<'_>> {
    children(node).into_iter().next()
}

// Las llaves que mira LeftCurly: las de los tipos y el switch, y las de los bloques que son cuerpo
// de un método, un control de flujo, un catch, una lambda o un case. Un bloque suelto o de
// inicialización no entra.
fn left_curlies(node: Node<'_>, lines: &Lines<'_>) -> Vec<LintFinding> {
    if TYPE_BODIES.contains(&node.kind()) {
        return first_child(node)
            .filter(|brace| brace.kind() == OPEN_CURLY)
            .and_then(|brace| misplaced_brace(brace, None, lines))
            .into_iter()
            .collect();
    }
    owned_blocks(node)
        .into_iter()
        .filter_map(|block| {
            let brace = first_child(block).filter(|brace| brace.kind() == OPEN_CURLY)?;
            misplaced_brace(brace, Some(block), lines)
        })
        .collect()
}

fn owned_blocks(node: Node<'_>) -> Vec<Node<'_>> {
    let kind = node.kind();
    let candidates: Vec<Node<'_>> = if BODY_OWNERS.contains(&kind) {
        node.child_by_field_name(BODY_FIELD).into_iter().collect()
    } else if kind == IF {
        [CONSEQUENCE_FIELD, ALTERNATIVE_FIELD]
            .iter()
            .filter_map(|field| node.child_by_field_name(field))
            .collect()
    } else if BLOCK_CHILD_OWNERS.contains(&kind) {
        named_children(node)
            .into_iter()
            .filter(|child| child.kind() == BLOCK)
            .take(1)
            .collect()
    } else if kind == SWITCH_GROUP {
        statements(node).into_iter().take(1).collect()
    } else {
        Vec::new()
    };
    candidates
        .into_iter()
        .filter(|block| block.kind() == BLOCK || block.kind() == CONSTRUCTOR_BODY)
        .collect()
}

// Calca LeftCurly con eol: la llave no empieza su línea, y la de un bloque no tiene la primera
// sentencia en su misma línea. `{}` pegadas no se miran.
fn misplaced_brace(brace: Node<'_>, block: Option<Node<'_>>, lines: &Lines<'_>) -> Option<LintFinding> {
    let end = brace.end_position();
    if lines.byte(end.row, end.column) == Some(b'}') {
        return None;
    }
    let first_line = lines.is_blank_before(brace.start_position());
    let shares_line = block
        .and_then(|block| statements(block).into_iter().next())
        .is_some_and(|statement| statement.start_position().row == brace.start_position().row);
    (first_line || shares_line).then(|| finding(LintKind::Java(JavaRule::LeftCurly), brace, OPEN_CURLY, None))
}

// Lo que sigue a una sentencia en el árbol de Checkstyle: la sentencia de al lado, la llave que
// cierra el bloque o lo que sigue al bloque que la contiene.
#[derive(Clone, Copy)]
struct Next {
    row: usize,
    is_semicolon: bool,
}

impl Next {
    fn of(node: Node<'_>) -> Self {
        Self {
            row: node.start_position().row,
            is_semicolon: node.kind() == SEMICOLON,
        }
    }
}

fn switch_groups(switch_block: Node<'_>, lines: &Lines<'_>) -> Vec<LintFinding> {
    let groups: Vec<Node<'_>> = named_children(switch_block)
        .into_iter()
        .filter(|child| child.kind() == SWITCH_GROUP)
        .collect();
    let closing = last_child(switch_block).filter(|child| child.kind() == CLOSE_CURLY);
    groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| {
            let after = groups
                .get(index.saturating_add(1))
                .copied()
                .or(closing)
                .map(Next::of);
            right_curlies_in(&statements(*group), after, lines)
        })
        .collect()
}

fn right_curlies_in(statements: &[Node<'_>], tail: Option<Next>, lines: &Lines<'_>) -> Vec<LintFinding> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(index, statement)| {
            let next = statements
                .get(index.saturating_add(1))
                .copied()
                .map(Next::of)
                .or(tail);
            multi_block_curlies(*statement, next, lines)
        })
        .collect()
}

fn multi_block_curlies(statement: Node<'_>, next: Option<Next>, lines: &Lines<'_>) -> Vec<LintFinding> {
    let kind = statement.kind();
    if kind == IF {
        return if_curlies(statement, next, lines);
    }
    if TRY_STATEMENTS.contains(&kind) {
        return try_curlies(statement, next, lines);
    }
    if NESTING_STATEMENTS.contains(&kind) {
        return statement
            .child_by_field_name(BODY_FIELD)
            .or_else(|| statements(statement).into_iter().last())
            .filter(|body| body.kind() == IF || TRY_STATEMENTS.contains(&body.kind()))
            .map(|body| multi_block_curlies(body, next, lines))
            .unwrap_or_default();
    }
    Vec::new()
}

fn if_curlies(statement: Node<'_>, next: Option<Next>, lines: &Lines<'_>) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let else_keyword = children(statement).into_iter().find(|child| child.kind() == ELSE);
    let consequence = statement.child_by_field_name(CONSEQUENCE_FIELD);
    let alternative = statement.child_by_field_name(ALTERNATIVE_FIELD);
    if let Some(consequence) = consequence {
        let after_consequence = else_keyword.map(Next::of).or(next);
        if consequence.kind() == BLOCK {
            findings.extend(closing_curly(
                consequence,
                after_consequence,
                else_keyword.is_none(),
                lines,
            ));
        } else {
            findings.extend(multi_block_curlies(consequence, after_consequence, lines));
        }
    }
    if let Some(alternative) = alternative {
        if alternative.kind() == BLOCK {
            findings.extend(closing_curly(alternative, next, true, lines));
        } else {
            findings.extend(multi_block_curlies(alternative, next, lines));
        }
    }
    findings
}

fn try_curlies(statement: Node<'_>, next: Option<Next>, lines: &Lines<'_>) -> Vec<LintFinding> {
    let mut parts: Vec<(Node<'_>, Node<'_>)> = Vec::new();
    if let Some(body) = statement.child_by_field_name(BODY_FIELD) {
        parts.push((body, body));
    }
    for clause in named_children(statement) {
        let block = match clause.kind() {
            CATCH => clause.child_by_field_name(BODY_FIELD),
            FINALLY => named_children(clause)
                .into_iter()
                .find(|child| child.kind() == BLOCK),
            _ => None,
        };
        if let Some(block) = block {
            parts.push((clause, block));
        }
    }
    parts
        .iter()
        .enumerate()
        .filter_map(|(index, (_, block))| {
            let following = parts
                .get(index.saturating_add(1))
                .map(|(clause, _)| Next::of(*clause));
            closing_curly(*block, following.or(next), following.is_none(), lines)
        })
        .collect()
}

// Calca RightCurly con same para if, else, try, catch y finally: la llave que cierra no comparte
// línea con la última sentencia; antes de else, catch o finally va en su misma línea; y la última
// va sola en su línea, salvo que todo el bloque quepa en una.
fn closing_curly(
    block: Node<'_>,
    next: Option<Next>,
    is_last: bool,
    lines: &Lines<'_>,
) -> Option<LintFinding> {
    let opening = first_child(block).filter(|brace| brace.kind() == OPEN_CURLY)?;
    let closing = last_child(block).filter(|brace| brace.kind() == CLOSE_CURLY)?;
    let open_row = opening.start_position().row;
    let close_row = closing.start_position().row;
    let crowded = statements(block)
        .into_iter()
        .last()
        .is_some_and(|statement| statement_row(statement) == close_row);
    let next_on_line = next.is_some_and(|next| next.row == close_row);
    let misplaced = if crowded && open_row != close_row {
        true
    } else if !is_last {
        !next_on_line
    } else {
        let alone = !next_on_line && lines.is_blank_before(closing.start_position());
        let single_line =
            open_row == close_row && (!next_on_line || next.is_some_and(|next| next.is_semicolon));
        !alone && !single_line
    };
    misplaced.then(|| finding(LintKind::Java(JavaRule::RightCurly), closing, CLOSE_CURLY, None))
}

// La línea con la que Checkstyle ubica una sentencia al lado de la llave: el `;` de una expresión o
// una declaración (va aparte en su árbol) y el principio de las demás.
fn statement_row(statement: Node<'_>) -> usize {
    let position: Point = if SEMICOLON_ENDED.contains(&statement.kind()) {
        statement.end_position()
    } else {
        statement.start_position()
    };
    position.row
}
