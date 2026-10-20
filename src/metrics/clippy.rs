mod cognitive;
mod lines;
mod macros;
mod parameters;

use std::sync::LazyLock;

use regex::{Captures, Regex};
use tree_sitter::Node;

use super::{FunctionMetrics, unmeasured};
use crate::syntax::{SnippetLanguage, children_with_fields, descendants, one_based, parse_snippet};

const FUNCTION: &str = "function_item";
const SIGNATURE: &str = "function_signature_item";
const CLOSURE: &str = "closure_expression";
const MACRO: &str = "macro_invocation";
const MACRO_DEFINITION: &str = "macro_definition";
const MACRO_RULE: &str = "macro_rule";
const RIGHT_FIELD: &str = "right";
const CONTAINERS: &[&str] = &["impl_item", "trait_item"];
const PLACEHOLDER_PREFIX: char = '_';
static METAVARIABLE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").ok());
const REPETITION_START: &[u8] = b"$(";
const REPETITION_OPERATORS: &[u8] = b"*+?";
const OPENING: u8 = b'(';
const CLOSING: u8 = b')';
const BLANK_BYTE: u8 = b' ';
const MODIFIERS: &str = "function_modifiers";

pub(super) fn measure(node: Node<'_>, source: &[u8]) -> Vec<FunctionMetrics> {
    match node.kind() {
        FUNCTION => vec![FunctionMetrics {
            lines: lines::too_many_lines(node, source),
            cognitive: cognitive::of_function(node, source),
            parameters: parameters::too_many_arguments(node, source),
            ..unmeasured(node, source)
        }],
        SIGNATURE => vec![FunctionMetrics {
            parameters: parameters::too_many_arguments(node, source),
            ..unmeasured(node, source)
        }],
        CLOSURE => vec![closure(node, source)],
        MACRO => closures_in_macro(node, source, 0),
        MACRO_DEFINITION => functions_in_macro_rules(node, source),
        _ => Vec::new(),
    }
}

fn closure(node: Node<'_>, source: &[u8]) -> FunctionMetrics {
    FunctionMetrics {
        cognitive: cognitive::of_closure(node, source),
        ..unmeasured(node, source)
    }
}

// Un closure pasado a una macro es código del usuario que clippy mide aparte después de expandir.
// tree-sitter no lo ve: se busca en los argumentos compilados aparte y se lleva la línea al
// archivo original.
fn closures_in_macro(invocation: Node<'_>, source: &[u8], origin_row: usize) -> Vec<FunctionMetrics> {
    macros::expand(invocation, source)
        .arguments
        .iter()
        .flat_map(|argument| {
            let argument_row = origin_row.saturating_add(argument.first_row);
            let text = argument.text.as_bytes();
            argument
                .expression()
                .map(descendants)
                .unwrap_or_default()
                .into_iter()
                .flat_map(move |node| match node.kind() {
                    CLOSURE => {
                        let row = argument_row.saturating_add(node.start_position().row);
                        vec![FunctionMetrics {
                            line: one_based(row),
                            end_line: one_based(argument_row.saturating_add(node.end_position().row)),
                            ..closure(node, text)
                        }]
                    }
                    MACRO => closures_in_macro(node, text, argument_row),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

// clippy mide el largo y los parámetros de las funciones que arma una `macro_rules!` local, en la
// línea de la definición; la complejidad y el anidamiento no, porque ese cuerpo viene de una
// expansión. Se da por hecho que la macro se usa en algún lado del crate. Los parámetros se miden
// sólo si el `impl` o el `trait` está en la misma macro: una que arma métodos sueltos suele usarse
// adentro de la implementación de un trait, que clippy no mira.
fn functions_in_macro_rules(definition: Node<'_>, source: &[u8]) -> Vec<FunctionMetrics> {
    children_of_kind(definition, MACRO_RULE)
        .into_iter()
        .filter_map(|rule| rule.child_by_field_name(RIGHT_FIELD))
        .flat_map(|transcriber| functions_in_transcriber(transcriber, source))
        .collect()
}

fn delimited_text<'source>(tree: Node<'_>, source: &'source [u8]) -> Option<(&'source str, usize)> {
    let tokens: Vec<Node<'_>> = children_with_fields(tree)
        .into_iter()
        .map(|(_, token)| token)
        .collect();
    let (open, close) = (tokens.first()?, tokens.last()?);
    let text = std::str::from_utf8(source.get(open.end_byte()..close.start_byte())?).ok()?;
    Some((text, open.start_position().row))
}

fn functions_in_transcriber(transcriber: Node<'_>, source: &[u8]) -> Vec<FunctionMetrics> {
    let Some((text, origin_row)) = delimited_text(transcriber, source) else {
        return Vec::new();
    };
    let readable = without_metavariables(text);
    let Some(tree) = parse_snippet(SnippetLanguage::Rust, &readable) else {
        return Vec::new();
    };
    let snippet = readable.as_bytes();
    descendants(tree.root_node())
        .into_iter()
        .filter(|node| node.kind() == FUNCTION && !node.has_error())
        .map(|function| FunctionMetrics {
            lines: lines::too_many_lines(function, text.as_bytes()),
            parameters: inside_implementation(function)
                .then(|| parameters::too_many_arguments(function, snippet))
                .flatten(),
            line: one_based(origin_row.saturating_add(function.start_position().row)),
            end_line: one_based(origin_row.saturating_add(function.end_position().row)),
            ..unmeasured(function, snippet)
        })
        .collect()
}

// tree-sitter no lee `$tipo` en posición de tipo ni las repeticiones `$( .. )*`: se cambian por
// texto del mismo largo, así las posiciones no se mueven y las líneas se cuentan sobre el original,
// que es lo que mide clippy.
fn without_metavariables(text: &str) -> String {
    let Some(metavariable) = METAVARIABLE.as_ref() else {
        return text.to_owned();
    };
    let marked = metavariable.replace_all(text, |found: &Captures<'_>| {
        format!(
            "{PLACEHOLDER_PREFIX}{}",
            found.get(1).map_or("", |name| name.as_str())
        )
    });
    let mut bytes = marked.into_owned().into_bytes();
    let mut search_from = 0;
    while let Some(start) = bytes
        .get(search_from..)
        .and_then(|rest| {
            rest.windows(REPETITION_START.len())
                .position(|pair| pair == REPETITION_START)
        })
        .map(|offset| search_from.saturating_add(offset))
    {
        blank_repetition(&mut bytes, start);
        search_from = start.saturating_add(REPETITION_START.len());
    }
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

fn blank_repetition(bytes: &mut [u8], start: usize) {
    let mut depth = 0_usize;
    let mut close = None;
    for (index, &byte) in bytes.iter().enumerate().skip(start.saturating_add(1)) {
        if byte == OPENING {
            depth = depth.saturating_add(1);
        } else if byte == CLOSING {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                close = Some(index);
                break;
            }
        }
    }
    let Some(close) = close else {
        return;
    };
    let after = close.saturating_add(1);
    let operator = (after..after.saturating_add(2)).find(|&index| {
        bytes
            .get(index)
            .is_some_and(|byte| REPETITION_OPERATORS.contains(byte))
    });
    let blanked = [start, start.saturating_add(1), close]
        .into_iter()
        .chain(operator.map_or(after..after, |operator| after..operator.saturating_add(1)));
    for index in blanked {
        if let Some(byte) = bytes.get_mut(index) {
            *byte = BLANK_BYTE;
        }
    }
}

fn inside_implementation(function: Node<'_>) -> bool {
    std::iter::successors(function.parent(), Node::parent)
        .any(|ancestor| CONTAINERS.contains(&ancestor.kind()))
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
