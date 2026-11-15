use tree_sitter::{Node, Point};

use super::java::{children, text};
use super::{LintFinding, LintKind, finding_at};
use crate::rules::JavaRule;
use crate::syntax::descendants;

const TEXT_BLOCK_RELEASE: u32 = 15;
const BINARY: &str = "binary_expression";
const OPERATOR_FIELD: &str = "operator";
const LEFT_FIELD: &str = "left";
const CONCATENATION: &str = "+";
const STRING_LITERAL: &str = "string_literal";
const TEXT_BLOCK_QUOTES: &str = "\"\"\"";
const COMMENT_SUFFIX: &str = "comment";
const LITERALS: [&str; 11] = [
    STRING_LITERAL,
    "character_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "true",
    "false",
    "null_literal",
];
const ESCAPE: u8 = b'\\';
const NEW_LINE_ESCAPE: u8 = b'n';
const NEW_LINES_FOR_A_BLOCK: usize = 2;

pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < TEXT_BLOCK_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| is_concatenation(*node, source) && !continues_a_concatenation(*node, source))
        .filter_map(|concatenation| {
            let first_new_line = first_new_line_of_a_block(concatenation, source)?;
            Some(finding_at(
                LintKind::Java(JavaRule::TextBlockMigration),
                first_new_line,
                text(concatenation, source),
                None,
            ))
        })
        .collect()
}

fn is_concatenation(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == BINARY
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == CONCATENATION)
}

fn continues_a_concatenation(node: Node<'_>, source: &[u8]) -> bool {
    node.parent().is_some_and(|parent| {
        is_concatenation(parent, source)
            && parent
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.id() == node.id())
    })
}

fn first_new_line_of_a_block(concatenation: Node<'_>, source: &[u8]) -> Option<Point> {
    let (operands, has_comments) = flattened(concatenation, source);
    let all_plain_literals = operands
        .iter()
        .all(|operand| LITERALS.contains(&operand.kind()) && !is_text_block(*operand, source));
    let is_string = operands.iter().any(|operand| operand.kind() == STRING_LITERAL);
    if !all_plain_literals || !is_string || has_comments {
        return None;
    }
    let mut first = None;
    let mut new_lines = 0;
    for operand in &operands {
        let offsets = new_line_offsets(text(*operand, source).as_bytes());
        if first.is_none() {
            first = offsets.first().map(|offset| {
                let start = operand.start_position();
                Point::new(start.row, start.column + offset)
            });
        }
        new_lines += offsets.len();
    }
    (new_lines >= NEW_LINES_FOR_A_BLOCK).then_some(first).flatten()
}

fn flattened<'tree>(concatenation: Node<'tree>, source: &[u8]) -> (Vec<Node<'tree>>, bool) {
    let mut operands = Vec::new();
    let mut has_comments = false;
    collect_operands(concatenation, source, &mut operands, &mut has_comments);
    (operands, has_comments)
}

fn collect_operands<'tree>(
    concatenation: Node<'tree>,
    source: &[u8],
    operands: &mut Vec<Node<'tree>>,
    has_comments: &mut bool,
) {
    let left = concatenation.child_by_field_name(LEFT_FIELD);
    for child in children(concatenation) {
        if child.kind().ends_with(COMMENT_SUFFIX) {
            *has_comments = true;
        } else if left.is_some_and(|left| left.id() == child.id()) && is_concatenation(child, source) {
            collect_operands(child, source, operands, has_comments);
        } else if child.is_named() {
            operands.push(child);
        }
    }
}

fn is_text_block(literal: Node<'_>, source: &[u8]) -> bool {
    text(literal, source).starts_with(TEXT_BLOCK_QUOTES)
}

fn new_line_offsets(written: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut index = 0;
    while index < written.len() {
        if written.get(index) == Some(&ESCAPE) {
            if written.get(index + 1) == Some(&NEW_LINE_ESCAPE) {
                offsets.push(index);
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    offsets
}
