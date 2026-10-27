use tree_sitter::Node;

use super::imports::Imports;
use super::parentheses::without_parentheses;
use crate::syntax::{descendants, named_children};

const CALL: &str = "call_expression";
const SELECTOR: &str = "selector_expression";
const IDENTIFIER: &str = "identifier";
const UNARY: &str = "unary_expression";
const INT_LITERAL: &str = "int_literal";
const COMMENT: &str = "comment";
const MINUS: &str = "-";
const ONE: &str = "1";
const FUNCTION_FIELD: &str = "function";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERAND_FIELD: &str = "operand";
const OPERATOR_FIELD: &str = "operator";
const FIELD_FIELD: &str = "field";
// Las de staticcheck, con la que reemplaza a cada una cuando el último argumento es `-1`.
const UNLIMITED_CALLS: &[(&str, &str, &str)] = &[
    ("strings", "Replace", "strings.ReplaceAll"),
    ("strings", "SplitN", "strings.Split"),
    ("strings", "SplitAfterN", "strings.SplitAfter"),
    ("bytes", "Replace", "bytes.ReplaceAll"),
    ("bytes", "SplitN", "bytes.Split"),
    ("bytes", "SplitAfterN", "bytes.SplitAfter"),
];

// QF1004 de staticcheck: `strings.Replace(s, a, b, -1)` es `strings.ReplaceAll(s, a, b)`. Devuelve
// la función llamada y la que la reemplaza.
pub fn unlimited_calls<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<(Node<'tree>, &'static str)> {
    let imports = Imports::of(root, source);
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == CALL)
        .filter(|call| last_argument_is_minus_one(*call, source))
        .filter_map(|call| {
            let function = call.child_by_field_name(FUNCTION_FIELD)?;
            let replacement = replacement(function, &imports, source)?;
            Some((function, replacement))
        })
        .collect()
}

fn last_argument_is_minus_one(call: Node<'_>, source: &[u8]) -> bool {
    let text = |node: Node<'_>| node.utf8_text(source).unwrap_or_default();
    call.child_by_field_name(ARGUMENTS_FIELD)
        .and_then(|arguments| {
            named_children(arguments)
                .into_iter()
                .rfind(|argument| argument.kind() != COMMENT)
        })
        .filter(|last| last.kind() == UNARY)
        .is_some_and(|last| {
            last.child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator) == MINUS)
                && last
                    .child_by_field_name(OPERAND_FIELD)
                    .is_some_and(|operand| operand.kind() == INT_LITERAL && text(operand) == ONE)
        })
}

fn replacement(function: Node<'_>, imports: &Imports, source: &[u8]) -> Option<&'static str> {
    let function = without_parentheses(function);
    if function.kind() != SELECTOR {
        return None;
    }
    let package = function
        .child_by_field_name(OPERAND_FIELD)
        .filter(|operand| operand.kind() == IDENTIFIER)?;
    let path = imports.package(package.utf8_text(source).ok()?)?;
    let name = function
        .child_by_field_name(FIELD_FIELD)?
        .utf8_text(source)
        .ok()?;
    UNLIMITED_CALLS
        .iter()
        .find(|(call_path, call_name, _)| *call_path == path && *call_name == name)
        .map(|(_, _, replacement)| *replacement)
}
