use tree_sitter::Node;

use super::nodes::COMMENT;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const ARRAY: &str = "array";
const ARRAY_PATTERN: &str = "array_pattern";
const OBJECT_PATTERN: &str = "object_pattern";
const COMMA: &str = ",";
const OPENING: &str = "[";
const CLOSING: &str = "]";

// Los elementos como los ve ESTree: un hueco (`[a, , b]`) es `None`; la coma final no suma nada.
pub fn elements_with_holes(array: Node<'_>) -> Vec<Option<Node<'_>>> {
    let mut elements = Vec::new();
    let mut expecting_element = false;
    let mut cursor = array.walk();
    for child in array.children(&mut cursor) {
        match child.kind() {
            COMMENT => {}
            OPENING => expecting_element = true,
            CLOSING => break,
            COMMA => {
                if expecting_element {
                    elements.push(None);
                }
                expecting_element = true;
            }
            _ => {
                elements.push(Some(child));
                expecting_element = false;
            }
        }
    }
    elements
}

// `no-sparse-arrays`: cada hueco de un arreglo literal, marcado en la coma que lo cierra.
pub fn sparse_arrays(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[ARRAY])
        .flat_map(hole_commas)
        .map(|comma| report(EslintRule::NoSparseArrays, comma, ""))
        .collect()
}

fn hole_commas(array: Node<'_>) -> Vec<Node<'_>> {
    let mut commas = Vec::new();
    let mut previous: Option<&str> = None;
    let mut cursor = array.walk();
    for child in array
        .children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
    {
        if child.kind() == COMMA && previous.is_some_and(|kind| [OPENING, COMMA].contains(&kind)) {
            commas.push(child);
        }
        previous = Some(child.kind());
    }
    commas
}

// `no-empty-pattern`: una desestructuración que no saca nada (`{}` o `[]`).
pub fn empty_patterns(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[OBJECT_PATTERN, ARRAY_PATTERN])
        .filter(|pattern| elements_with_holes_or_properties(*pattern) == 0)
        .map(|pattern| report(EslintRule::NoEmptyPattern, pattern, ""))
        .collect()
}

fn elements_with_holes_or_properties(pattern: Node<'_>) -> usize {
    if pattern.kind() == ARRAY_PATTERN {
        return elements_with_holes(pattern).len();
    }
    let mut cursor = pattern.walk();
    pattern
        .named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .count()
}
