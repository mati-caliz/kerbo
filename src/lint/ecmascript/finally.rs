use tree_sitter::Node;

use super::nodes::text;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const RETURN: &str = "return_statement";
const THROW: &str = "throw_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const LABELED: &str = "labeled_statement";
const FINALLY: &str = "finally_clause";
const LABEL_FIELD: &str = "label";
const FUNCTIONS_AND_CLASSES: &[&str] = &[
    "program",
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
    "class_declaration",
    "abstract_class_declaration",
    "class",
];
const LOOPS: &[&str] = &[
    "do_statement",
    "while_statement",
    "for_in_statement",
    "for_statement",
];
const SWITCH: &str = "switch_statement";

// `no-unsafe-finally`: un `return`, `throw`, `break` o `continue` que sale de un `finally`.
pub fn unsafe_finally(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[RETURN, THROW, BREAK, CONTINUE])
        .filter(|statement| leaves_finally(*statement, context.source))
        .map(|statement| report(EslintRule::NoUnsafeFinally, statement, statement.kind()))
        .collect()
}

fn leaves_finally(statement: Node<'_>, source: &[u8]) -> bool {
    let label = statement
        .child_by_field_name(LABEL_FIELD)
        .map(|label| text(label, source));
    let mut label_inside = false;
    let mut current = statement;
    while !is_boundary(statement.kind(), label.is_some(), current.kind()) {
        let Some(parent) = current.parent() else {
            return false;
        };
        if parent.kind() == LABELED
            && label.is_some()
            && parent
                .child_by_field_name(LABEL_FIELD)
                .map(|own| text(own, source))
                == label
        {
            label_inside = true;
        }
        if parent.kind() == FINALLY {
            return !label_inside;
        }
        current = parent;
    }
    false
}

fn is_boundary(statement: &str, labeled: bool, kind: &str) -> bool {
    if FUNCTIONS_AND_CLASSES.contains(&kind) {
        return true;
    }
    match statement {
        BREAK if !labeled => LOOPS.contains(&kind) || kind == SWITCH,
        CONTINUE if !labeled => LOOPS.contains(&kind),
        _ => false,
    }
}
