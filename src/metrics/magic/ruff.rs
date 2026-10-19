use tree_sitter::Node;

use super::number::numeric_value;
use super::{named_children, operator, text};

const COMPARISON: &str = "comparison_operator";
const COMMENT: &str = "comment";
const PARENTHESIZED: &str = "parenthesized_expression";
const UNARY: &str = "unary_operator";
const LITERAL_SIGNS: &[&str] = &["+", "-", "~"];
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const STRING: &str = "string";
const CONCATENATED_STRING: &str = "concatenated_string";
const STRING_START: &str = "string_start";
const INTERPOLATED_PREFIXES: &[char] = &['f', 'F', 't', 'T'];
const NAMED_CONSTANTS: &[&str] = &["true", "false", "none", "ellipsis"];
const COMPLEX_SUFFIX: char = 'j';
const ALLOWED_VALUES: &[f64] = &[0.0, 1.0];
const SUBSCRIPT: &str = "subscript";
const ATTRIBUTE: &str = "attribute";
const SYS: &str = "sys";
const VERSION_ATTRIBUTES: &[&str] = &["version", "version_info"];
const IMPLEMENTATION: &str = "implementation";
const IMPLEMENTATION_VERSION: &str = "version";

#[derive(Clone, Copy)]
enum LiteralKind {
    Number,
    Other,
}

pub fn reported<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    if node.kind() != COMPARISON {
        return Vec::new();
    }
    let operands: Vec<Node<'tree>> = named_children(node)
        .into_iter()
        .filter(|operand| operand.kind() != COMMENT)
        .collect();
    let literals: Vec<Option<(LiteralKind, Node<'tree>)>> =
        operands.iter().map(|&operand| literal(operand, source)).collect();
    if literals.windows(2).any(|pair| pair.iter().all(Option::is_some)) {
        return Vec::new();
    }
    let is_version = |index: Option<usize>| {
        index
            .and_then(|index| operands.get(index))
            .is_some_and(|&operand| is_sys_version(operand, source))
    };
    operands
        .iter()
        .zip(&literals)
        .enumerate()
        .filter(|&(index, (_, literal))| {
            literal.is_some_and(|(kind, value)| is_magic(kind, value, source))
                && !is_version(index.checked_sub(1))
                && !is_version(index.checked_add(1))
        })
        .map(|(_, (&operand, _))| operand)
        .collect()
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        let Some(inner) = named_children(current)
            .into_iter()
            .find(|inner| inner.kind() != COMMENT)
        else {
            break;
        };
        current = inner;
    }
    current
}

fn literal<'tree>(operand: Node<'tree>, source: &[u8]) -> Option<(LiteralKind, Node<'tree>)> {
    let expression = unparenthesized(operand);
    let value = if expression.kind() == UNARY && LITERAL_SIGNS.contains(&operator(expression, source)) {
        unparenthesized(expression.child_by_field_name("argument")?)
    } else {
        expression
    };
    literal_kind(value, source).map(|kind| (kind, value))
}

fn literal_kind(node: Node<'_>, source: &[u8]) -> Option<LiteralKind> {
    match node.kind() {
        INTEGER | FLOAT => Some(LiteralKind::Number),
        STRING => (!is_interpolated(node, source)).then_some(LiteralKind::Other),
        CONCATENATED_STRING => named_children(node)
            .into_iter()
            .filter(|part| part.kind() == STRING)
            .all(|part| !is_interpolated(part, source))
            .then_some(LiteralKind::Other),
        kind if NAMED_CONSTANTS.contains(&kind) => Some(LiteralKind::Other),
        _ => None,
    }
}

fn is_interpolated(string: Node<'_>, source: &[u8]) -> bool {
    named_children(string)
        .into_iter()
        .find(|part| part.kind() == STRING_START)
        .is_some_and(|start| text(start, source).contains(INTERPOLATED_PREFIXES))
}

fn is_magic(kind: LiteralKind, value: Node<'_>, source: &[u8]) -> bool {
    if matches!(kind, LiteralKind::Other) {
        return false;
    }
    let number = text(value, source).to_ascii_lowercase();
    if number.ends_with(COMPLEX_SUFFIX) {
        return true;
    }
    numeric_value(&number).is_none_or(|parsed| {
        !ALLOWED_VALUES
            .iter()
            .any(|&allowed| (parsed - allowed).abs() < f64::EPSILON)
    })
}

fn is_sys_version(operand: Node<'_>, source: &[u8]) -> bool {
    let expression = unparenthesized(operand);
    let target = if expression.kind() == SUBSCRIPT {
        expression
            .child_by_field_name("value")
            .map_or(expression, unparenthesized)
    } else {
        expression
    };
    if target.kind() != ATTRIBUTE {
        return false;
    }
    let dotted: String = text(target, source)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let segments: Vec<&str> = dotted.split('.').collect();
    match segments.as_slice() {
        [SYS, attribute, ..] if VERSION_ATTRIBUTES.contains(attribute) => true,
        [SYS, IMPLEMENTATION, IMPLEMENTATION_VERSION, ..] => true,
        _ => false,
    }
}
