use tree_sitter::{Node, Point};

use super::parentheses::without_parentheses;
use crate::syntax::{descendants, named_children};

const SELECTOR: &str = "selector_expression";
const CALL: &str = "call_expression";
const COMMENT: &str = "comment";
const OPERAND_FIELD: &str = "operand";
const FIELD_FIELD: &str = "field";
const FUNCTION_FIELD: &str = "function";
const ARGUMENTS_FIELD: &str = "arguments";
const WRITE_METHODS: &[&str] = &["Write", "WriteString"];

// QF1008 sin tipos: cada campo que no es el último de una cadena `a.b.c` podría ser un embebido
// que sobra; los tipos dicen cuáles.
pub fn embedded_field_candidates<'tree>(root: Node<'tree>, source: &'tree [u8]) -> Vec<(Point, &'tree str)> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == SELECTOR)
        .filter(|selector| {
            selector.parent().is_some_and(|parent| {
                parent.kind() == SELECTOR
                    && parent
                        .child_by_field_name(OPERAND_FIELD)
                        .is_some_and(|operand| operand.id() == selector.id())
            })
        })
        .filter_map(|selector| {
            let field = selector.child_by_field_name(FIELD_FIELD)?;
            Some((field.start_position(), field.utf8_text(source).ok()?))
        })
        .collect()
}

// QF1012 sin tipos: `x.Write(...)` y `x.WriteString(...)` con un solo argumento; los tipos dicen si
// el argumento es un fmt.Sprint* y `x` un io.Writer.
pub fn fprint_candidates(root: Node<'_>, source: &[u8]) -> Vec<Point> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == CALL)
        .filter(|call| {
            let method = call
                .child_by_field_name(FUNCTION_FIELD)
                .map(without_parentheses)
                .filter(|function| function.kind() == SELECTOR)
                .and_then(|function| function.child_by_field_name(FIELD_FIELD))
                .and_then(|field| field.utf8_text(source).ok());
            let single_argument = call
                .child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| {
                    named_children(arguments)
                        .iter()
                        .filter(|argument| argument.kind() != COMMENT)
                        .count()
                        == 1
                });
            method.is_some_and(|method| WRITE_METHODS.contains(&method)) && single_argument
        })
        .map(|call| call.start_position())
        .collect()
}
