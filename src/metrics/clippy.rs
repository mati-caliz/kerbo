pub(super) mod cfg;
mod cognitive;
mod lines;
pub(super) mod macros;
mod parameters;

use std::collections::HashSet;

use tree_sitter::Node;

use super::FunctionMetrics;
use crate::macro_rules::{Code, Expansion, MacroScope};
use crate::syntax::{children_with_fields, function_name, one_based};

const FUNCTION: &str = "function_item";
const SIGNATURE: &str = "function_signature_item";
const CLOSURE: &str = "closure_expression";
const MACRO: &str = "macro_invocation";
const MODIFIERS: &str = "function_modifiers";
const BODY_FIELD: &str = "body";

type MetricKey = (usize, usize, usize, Option<usize>, Option<usize>, Option<usize>);

pub(super) fn measure_file(root: Node<'_>, scope: &MacroScope<'_>) -> Vec<FunctionMetrics> {
    let code = Code::root(scope.current_text(), scope);
    if cfg::file_is_compiled_out(root, &code) {
        return Vec::new();
    }
    let mut functions = measure_tree(root, &code);
    for invocation in scope.foreign_invocations() {
        let Some(text) = scope.source_text(invocation.source) else {
            continue;
        };
        let foreign = Code::of_source(text, invocation.source, scope);
        if let Some(expansion) = foreign.expand_recorded(invocation) {
            functions.extend(in_expansion(&expansion, scope));
        }
    }
    let mut seen: HashSet<MetricKey> = HashSet::new();
    functions.retain(|function| {
        seen.insert((
            function.line,
            function.column,
            function.end_line,
            function.lines,
            function.cognitive,
            function.parameters,
        ))
    });
    functions
}

fn measure_tree(root: Node<'_>, code: &Code<'_>) -> Vec<FunctionMetrics> {
    let mut functions = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if cfg::is_compiled_out(node, code) {
            continue;
        }
        functions.extend(measure(node, code));
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    functions
}

fn measure(node: Node<'_>, code: &Code<'_>) -> Vec<FunctionMetrics> {
    match node.kind() {
        FUNCTION => function(node, code),
        SIGNATURE => located(node, code)
            .map(|located| FunctionMetrics {
                parameters: parameters::too_many_arguments(node, code.bytes()),
                ..located
            })
            .into_iter()
            .collect(),
        CLOSURE => closure(node, code).into_iter().collect(),
        MACRO => code.expand_node(node).map_or_else(
            || closures_in_macro(node, code),
            |expansion| in_expansion(&expansion, code.scope()),
        ),
        _ => Vec::new(),
    }
}

fn in_expansion(expansion: &Expansion, scope: &MacroScope<'_>) -> Vec<FunctionMetrics> {
    let code = expansion.code(scope);
    expansion
        .content()
        .map(|content| measure_tree(content, &code))
        .unwrap_or_default()
}

// clippy reporta el largo y los parámetros donde arranca la función, también si ese texto viene de
// una `macro_rules!` (entonces en la definición); la complejidad, sólo si la función entera está
// escrita en el archivo y no sale de una expansión. Cada valor va aparte porque rustc no repite un
// aviso idéntico, y una macro que arma la misma función en cada invocación da un solo aviso.
fn function(node: Node<'_>, code: &Code<'_>) -> Vec<FunctionMetrics> {
    let Some(located) = located(node, code) else {
        return Vec::new();
    };
    let lines = lines::too_many_lines(node, code);
    let parameters = parameters::too_many_arguments(node, code.bytes());
    if is_written_here(node, code) {
        return vec![FunctionMetrics {
            lines,
            cognitive: cognitive::of_function(node, code),
            parameters,
            ..located
        }];
    }
    [(lines, None), (None, parameters)]
        .into_iter()
        .filter(|(lines, parameters)| lines.is_some() || parameters.is_some())
        .map(|(lines, parameters)| FunctionMetrics {
            lines,
            parameters,
            ..clone_location(&located)
        })
        .collect()
}

fn closure(node: Node<'_>, code: &Code<'_>) -> Option<FunctionMetrics> {
    is_written_here(node, code).then_some(())?;
    Some(FunctionMetrics {
        cognitive: cognitive::of_closure(node, code),
        ..located(node, code)?
    })
}

fn is_written_here(node: Node<'_>, code: &Code<'_>) -> bool {
    node.end_byte()
        .checked_sub(1)
        .is_some_and(|last| code.is_written_here(node.start_byte(), last))
}

fn located(node: Node<'_>, code: &Code<'_>) -> Option<FunctionMetrics> {
    let header_end = node
        .child_by_field_name(BODY_FIELD)
        .map_or_else(|| node.end_byte(), |body| body.start_byte());
    let (row, column) = code.item_position(node.start_byte(), header_end)?;
    let end_row = node
        .end_byte()
        .checked_sub(1)
        .and_then(|last| code.position(last))
        .map_or(row, |(end_row, _)| end_row.max(row));
    Some(FunctionMetrics {
        name: function_name(node, code.bytes()),
        line: one_based(row),
        column: one_based(column),
        end_line: one_based(end_row),
        lines: None,
        cyclomatic: None,
        cognitive: None,
        parameters: None,
    })
}

fn clone_location(located: &FunctionMetrics) -> FunctionMetrics {
    FunctionMetrics {
        name: located.name.clone(),
        lines: None,
        cognitive: None,
        parameters: None,
        cyclomatic: None,
        ..*located
    }
}

// Un closure pasado a una macro que no es del crate es código del usuario que clippy mide aparte
// después de expandir. tree-sitter no lo ve: se buscan los argumentos que compilan como expresión.
fn closures_in_macro(invocation: Node<'_>, code: &Code<'_>) -> Vec<FunctionMetrics> {
    if !macros::may_hold_closures(invocation, code.bytes()) {
        return Vec::new();
    }
    macros::expand(invocation, code)
        .arguments
        .iter()
        .flat_map(|argument| {
            let argument_code = argument.code(code);
            argument
                .expression()
                .map(|expression| measure_tree(expression, &argument_code))
                .unwrap_or_default()
        })
        .collect()
}

fn has_modifier(function: Node<'_>, modifier: &str) -> bool {
    modifiers(function)
        .into_iter()
        .any(|modifier_node| modifier_node.kind() == modifier)
}

fn modifiers(function: Node<'_>) -> Vec<Node<'_>> {
    children_of_kind(function, MODIFIERS)
        .into_iter()
        .flat_map(|list| children_with_fields(list).into_iter().map(|(_, child)| child))
        .collect()
}

fn children_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    children_with_fields(node)
        .into_iter()
        .map(|(_, child)| child)
        .filter(|child| child.kind() == kind)
        .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expansion_tests;
