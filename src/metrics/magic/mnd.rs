use std::collections::HashSet;

use tree_sitter::Node;

use super::{named_children, text};

const LITERALS: &[&str] = &["int_literal", "float_literal"];
const DIGIT_SEPARATOR: char = '_';
const IGNORED_NUMBERS: &[&str] = &["0", "0.0", "1", "1.0"];
const IGNORED_FUNCTIONS: &[&str] = &[
    "time.Date",
    "strconv.FormatInt",
    "strconv.FormatUint",
    "strconv.FormatFloat",
    "strconv.ParseInt",
    "strconv.ParseUint",
    "strconv.ParseFloat",
];
const ANY_CHARACTER: char = '.';
const CONSTANT_SPEC: &str = "const_spec";
const CALL: &str = "call_expression";
const SELECTOR: &str = "selector_expression";
const IDENTIFIER: &str = "identifier";
const KEYED_ELEMENT: &str = "keyed_element";
const LITERAL_ELEMENT: &str = "literal_element";
const ASSIGNMENTS: &[&str] = &["assignment_statement", "short_var_declaration"];
const CASE: &str = "expression_case";
const IF: &str = "if_statement";
const PARENTHESIZED: &str = "parenthesized_expression";
const RETURN: &str = "return_statement";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const COMMENT: &str = "comment";

pub fn reported<'tree>(nodes: &[Node<'tree>], source: &[u8]) -> Vec<Node<'tree>> {
    let constant_rows: HashSet<usize> = nodes
        .iter()
        .filter(|node| node.kind() == CONSTANT_SPEC)
        .map(|node| node.start_position().row)
        .collect();
    let mut found = Vec::new();
    for &node in nodes {
        match node.kind() {
            CALL if !constant_rows.contains(&node.start_position().row) => {
                argument_check(node, source, &mut found);
            }
            KEYED_ELEMENT => {
                if let Some(value) = node.child_by_field_name("value").map(unwrap_element) {
                    literal_or_operands(value, source, &mut found);
                }
            }
            kind if ASSIGNMENTS.contains(&kind) => assignment_checks(node, source, &mut found),
            CASE => {
                for value in node
                    .child_by_field_name("value")
                    .map(expressions)
                    .unwrap_or_default()
                {
                    literal_or_operands(value, source, &mut found);
                }
            }
            IF => {
                if let Some(condition) = node
                    .child_by_field_name("condition")
                    .filter(|condition| condition.kind() == BINARY)
                {
                    operands(condition, source, &mut found);
                }
            }
            PARENTHESIZED => {
                if let Some(inner) = named_children(node)
                    .into_iter()
                    .find(|inner| inner.kind() == BINARY)
                {
                    operands(inner, source, &mut found);
                }
            }
            RETURN => {
                for result in named_children(node).into_iter().flat_map(expressions) {
                    literal_or_operands(result, source, &mut found);
                }
            }
            _ => {}
        }
    }
    found
}

fn argument_check<'tree>(call: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    let callee = call.child_by_field_name("function");
    if callee.is_some_and(|callee| is_ignored_function(callee, source)) {
        return;
    }
    for argument in call
        .child_by_field_name("arguments")
        .map(expressions)
        .unwrap_or_default()
    {
        literal_or_operands(argument, source, found);
    }
}

fn is_ignored_function(callee: Node<'_>, source: &[u8]) -> bool {
    let name = match callee.kind() {
        IDENTIFIER => text(callee, source).to_owned(),
        SELECTOR => {
            let Some(operand) = callee
                .child_by_field_name("operand")
                .filter(|operand| operand.kind() == IDENTIFIER)
            else {
                return false;
            };
            let field = callee
                .child_by_field_name("field")
                .map_or("", |field| text(field, source));
            format!("{}.{field}", text(operand, source))
        }
        _ => return false,
    };
    IGNORED_FUNCTIONS
        .iter()
        .any(|pattern| contains_pattern(&name, pattern))
}

fn contains_pattern(name: &str, pattern: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    name.windows(pattern.len()).any(|window| {
        window
            .iter()
            .zip(&pattern)
            .all(|(&character, &expected)| expected == ANY_CHARACTER || character == expected)
    })
}

fn assignment_checks<'tree>(assignment: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    for value in assignment
        .child_by_field_name("right")
        .map(expressions)
        .unwrap_or_default()
    {
        match value.kind() {
            UNARY => unary_operand(value, source, found),
            BINARY => binary_assignment(value, source, found),
            _ => {}
        }
    }
}

fn binary_assignment<'tree>(value: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    if let Some(right) = value
        .child_by_field_name("right")
        .filter(|right| right.kind() == UNARY)
    {
        unary_operand(right, source, found);
    }
    for side in ["left", "right"] {
        if let Some(nested) = value
            .child_by_field_name(side)
            .filter(|nested| nested.kind() == BINARY)
        {
            operands(nested, source, found);
        }
    }
    operands(value, source, found);
}

fn unary_operand<'tree>(unary: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    if let Some(operand) = unary.child_by_field_name("operand") {
        push_if_magic(operand, source, found);
    }
}

fn literal_or_operands<'tree>(expression: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    if expression.kind() == BINARY {
        operands(expression, source, found);
    } else {
        push_if_magic(expression, source, found);
    }
}

fn operands<'tree>(binary: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    for side in ["left", "right"] {
        if let Some(operand) = binary.child_by_field_name(side) {
            push_if_magic(operand, source, found);
        }
    }
}

fn push_if_magic<'tree>(node: Node<'tree>, source: &[u8], found: &mut Vec<Node<'tree>>) {
    if LITERALS.contains(&node.kind()) {
        let number: String = text(node, source)
            .chars()
            .filter(|&character| character != DIGIT_SEPARATOR)
            .collect();
        if !IGNORED_NUMBERS.contains(&number.as_str()) {
            found.push(node);
        }
    }
}

fn unwrap_element(element: Node<'_>) -> Node<'_> {
    if element.kind() == LITERAL_ELEMENT {
        named_children(element).into_iter().next().unwrap_or(element)
    } else {
        element
    }
}

fn expressions(list: Node<'_>) -> Vec<Node<'_>> {
    named_children(list)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}
