use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENT_FIELD, COMPARISON, FALSE, NONE, NOT_OPERATOR, OPERATORS_FIELD, TRUE, TUPLE, fields, is_f_string,
    is_string, named, text, unparenthesized,
};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::rules::PythonRule;

const EQUAL: &str = "==";
const NOT_EQUAL: &str = "!=";
const IS: &str = "is";
const IS_NOT: &str = "is not";
const IN: &str = "in";
const NOT_IN: &str = "not in";
const NUMBERS: [&str; 2] = ["integer", "float"];
const MUTABLE_INITIALIZERS: [&str; 6] = [
    "list",
    "set",
    "dictionary",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
];
const SPACE: &str = " ";

#[derive(Debug, Clone, Copy)]
struct Comparison<'list, 'tree> {
    node: Node<'tree>,
    operands: &'list [Node<'tree>],
    operators: &'list [Node<'tree>],
}

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    for node in nodes {
        match node.kind() {
            COMPARISON => {
                let operands = named(*node);
                let operators = fields(*node, OPERATORS_FIELD);
                let comparison = Comparison {
                    node: *node,
                    operands: &operands,
                    operators: &operators,
                };
                reports.extend(literal_comparisons(comparison, source));
                reports.extend(identity_with_literal(comparison, source));
            }
            NOT_OPERATOR => reports.extend(negated_test(*node, source)),
            _ => {}
        }
    }
    reports
}

fn literal_comparisons(comparison: Comparison<'_, '_>, source: &[u8]) -> Vec<Report> {
    let mut flagged: Vec<(usize, Report)> = Vec::new();
    let pairs = comparison
        .operators
        .iter()
        .zip(comparison.operands.iter().skip(1));
    if let (Some(left), Some((operator, next))) = (comparison.operands.first(), pairs.clone().next())
        && !is_constant_non_singleton(*next, source)
        && is_equality(*operator, source)
    {
        flagged.extend(singleton_report(comparison, *left).map(|report| (0, report)));
    }
    for (index, (operator, next)) in pairs.enumerate() {
        let Some(previous) = comparison.operands.get(index) else {
            continue;
        };
        if is_constant_non_singleton(*previous, source) || !is_equality(*operator, source) {
            continue;
        }
        flagged.extend(singleton_report(comparison, *next).map(|report| (index, report)));
    }
    let fix = identity_fix(comparison, &flagged, source);
    flagged
        .into_iter()
        .map(|(_, report)| report.with_fix(fix.clone()))
        .collect()
}

fn singleton_report(comparison: Comparison<'_, '_>, operand: Node<'_>) -> Option<Report> {
    match operand.kind() {
        NONE => Some(Report::at(PythonRule::E711, operand, "")),
        TRUE | FALSE => Some(Report::at(PythonRule::E712, comparison.node, "")),
        _ => None,
    }
}

fn is_equality(operator: Node<'_>, source: &[u8]) -> bool {
    matches!(text(operator, source), EQUAL | NOT_EQUAL)
}

// E711 y E712, inseguros como en ruff: `x == True` pasa a `x` y `x != True` a `not x`; si no, cada
// `==` o `!=` señalado pasa a `is` o `is not`.
fn identity_fix(comparison: Comparison<'_, '_>, flagged: &[(usize, Report)], source: &[u8]) -> Option<Fix> {
    if flagged.is_empty() {
        return None;
    }
    let fix =
        redundant_boolean(comparison, source).unwrap_or_else(|| operator_fix(comparison, flagged, source));
    Some(fix.marked_unsafe())
}

fn redundant_boolean(comparison: Comparison<'_, '_>, source: &[u8]) -> Option<Fix> {
    let ([left, right], [operator]) = (comparison.operands, comparison.operators) else {
        return None;
    };
    let equal = text(*operator, source) == EQUAL;
    let (literal, other, wrap) = if let Some(value) = boolean_literal(*left) {
        let wrap = unparenthesized(*left).start_byte() != comparison.node.start_byte();
        (value, *right, wrap)
    } else {
        let value = boolean_literal(*right)?;
        let wrap = unparenthesized(*right).end_byte() != comparison.node.end_byte();
        (value, *left, wrap)
    };
    let condition = text(other, source);
    let result = if literal == equal {
        condition.to_owned()
    } else {
        format!("not {condition}")
    };
    let result = if wrap { format!("({result})") } else { result };
    Some(Fix::replace(comparison.node, &result, source))
}

fn boolean_literal(operand: Node<'_>) -> Option<bool> {
    match unparenthesized(operand).kind() {
        TRUE => Some(true),
        FALSE => Some(false),
        _ => None,
    }
}

fn operator_fix(comparison: Comparison<'_, '_>, flagged: &[(usize, Report)], source: &[u8]) -> Fix {
    let mut indexes: Vec<usize> = flagged.iter().map(|(index, _)| *index).collect();
    indexes.dedup();
    let edits = indexes
        .into_iter()
        .filter_map(|index| comparison.operators.get(index))
        .map(|operator| {
            let replacement = if text(*operator, source) == EQUAL {
                IS
            } else {
                IS_NOT
            };
            spaced_edit(*operator, replacement, source)
        })
        .collect();
    Fix::from_edits(source, edits)
}

fn spaced_edit(operator: Node<'_>, replacement: &str, source: &[u8]) -> TextEdit {
    let before = operator
        .start_byte()
        .checked_sub(1)
        .and_then(|index| source.get(index));
    let after = source.get(operator.end_byte());
    let pad = |byte: Option<&u8>| {
        if byte.is_some_and(u8::is_ascii_whitespace) {
            ""
        } else {
            SPACE
        }
    };
    TextEdit {
        start: TextPoint::from_point(operator.start_position()),
        end: TextPoint::from_point(operator.end_position()),
        replacement: format!("{}{replacement}{}", pad(before), pad(after)),
    }
}

fn identity_with_literal(comparison: Comparison<'_, '_>, source: &[u8]) -> Vec<Report> {
    comparison
        .operators
        .iter()
        .enumerate()
        .filter(|(_, operator)| matches!(text(**operator, source), IS | IS_NOT))
        .filter(|(index, _)| {
            let sides = [
                comparison.operands.get(*index),
                comparison.operands.get(index.saturating_add(1)),
            ];
            sides
                .into_iter()
                .flatten()
                .any(|side| is_constant_non_singleton(*side, source) || is_mutable_initializer(*side))
        })
        .map(|(_, operator)| {
            let replacement = if text(*operator, source) == IS {
                EQUAL
            } else {
                NOT_EQUAL
            };
            let fix = Fix::from_edits(source, vec![spaced_edit(*operator, replacement, source)]);
            Report::at(PythonRule::F632, comparison.node, "").with_fix(Some(fix))
        })
        .collect()
}

fn is_mutable_initializer(node: Node<'_>) -> bool {
    MUTABLE_INITIALIZERS.contains(&unparenthesized(node).kind())
}

pub fn is_constant_non_singleton(node: Node<'_>, source: &[u8]) -> bool {
    let node = unparenthesized(node);
    if node.kind() == TUPLE {
        return named(node).iter().all(|element| is_constant(*element, source));
    }
    NUMBERS.contains(&node.kind()) || is_plain_string(node, source)
}

fn is_constant(node: Node<'_>, source: &[u8]) -> bool {
    matches!(unparenthesized(node).kind(), NONE | TRUE | FALSE | "ellipsis")
        || is_constant_non_singleton(node, source)
}

fn is_plain_string(node: Node<'_>, source: &[u8]) -> bool {
    if !is_string(node) {
        return false;
    }
    let parts = if node.kind() == super::ast::STRING {
        vec![node]
    } else {
        named(node)
    };
    parts.iter().all(|part| !is_f_string(*part, source))
}

fn negated_test(node: Node<'_>, source: &[u8]) -> Option<Report> {
    let operand = unparenthesized(node.child_by_field_name(ARGUMENT_FIELD)?);
    if operand.kind() != COMPARISON {
        return None;
    }
    let operators = fields(operand, OPERATORS_FIELD);
    let [operator] = operators.as_slice() else {
        return None;
    };
    let (rule, negated) = match text(*operator, source) {
        IN => (PythonRule::E713, NOT_IN),
        IS => (PythonRule::E714, IS_NOT),
        _ => return None,
    };
    let operands = named(operand);
    let [left, right] = operands.as_slice() else {
        return None;
    };
    let replacement = format!("{} {negated} {}", text(*left, source), text(*right, source));
    let fix = Fix::replace(node, &replacement, source);
    Some(Report::at(rule, operand, "").with_fix(Some(fix)))
}
