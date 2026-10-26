use tree_sitter::Node;

use super::Report;
use super::ast::{CONCATENATED_STRING, LIST, SET, SUBSCRIPT, TUPLE};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::rules::PythonRule;

const EXPRESSION_LIST: &str = "expression_list";
const SUBSCRIPT_FIELD: &str = "subscript";
const MINIMUM_SUBSCRIPT_TUPLE: usize = 2;

// ISC004: `["a" "b", "c"]`, una concatenación implícita sin paréntesis dentro de una lista, tupla o
// conjunto, que suele ser una coma olvidada.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| collection_elements(*node))
        .filter(|element| element.kind() == CONCATENATED_STRING)
        .map(|element| {
            let fix = Fix::from_edits(
                source,
                vec![
                    TextEdit {
                        start: TextPoint::from_point(element.start_position()),
                        end: TextPoint::from_point(element.start_position()),
                        replacement: String::from("("),
                    },
                    TextEdit {
                        start: TextPoint::from_point(element.end_position()),
                        end: TextPoint::from_point(element.end_position()),
                        replacement: String::from(")"),
                    },
                ],
            );
            Report::at(PythonRule::ISC004, element, "").with_fix(Some(fix.marked_unsafe()))
        })
        .collect()
}

fn collection_elements(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        LIST | SET | TUPLE | EXPRESSION_LIST => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor).collect()
        }
        SUBSCRIPT => {
            let mut cursor = node.walk();
            let slices: Vec<Node<'_>> = node
                .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
                .collect();
            if slices.len() >= MINIMUM_SUBSCRIPT_TUPLE {
                slices
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}
