mod argument_types;
mod braces;
mod fstring;
pub mod percent;
mod printf;
mod template;
mod unused_arguments;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, LEFT_FIELD,
    NAME_FIELD, OBJECT_FIELD, RIGHT_FIELD, STRING, TUPLE, is_string, named, text, unparenthesized,
};
use super::escapes::text_value;
use crate::rules::PythonRule;
use argument_types::mismatched_types;
use braces::BraceSummary;
pub use fstring::is_convertible_format_call;
use percent::{PercentError, PercentSummary};
use unused_arguments::{unused_arguments_fix, unused_keys_fix};

const BINARY: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const MODULO: &str = "%";
const FORMAT: &str = "format";
const DICTIONARY: &str = "dictionary";
const CALL_ARGUMENTS: &str = "argument_list";
const DICTIONARY_COMPREHENSION: &str = "dictionary_comprehension";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const LIST_SPLAT: &str = "list_splat";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const GENERATOR: &str = "generator_expression";
const SEQUENCES: [&str; 6] = [
    TUPLE,
    "list",
    "set",
    "list_comprehension",
    "set_comprehension",
    GENERATOR,
];
const SINGLE_VALUES: [&str; 10] = [
    "list",
    "set",
    "list_comprehension",
    "set_comprehension",
    GENERATOR,
    "integer",
    "float",
    "true",
    "false",
    "none",
];
const NEGATION: &str = "not_operator";
const UNARY: &str = "unary_operator";
const SEPARATOR: &str = ", ";

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            BINARY => percent_format(*node, source)
                .into_iter()
                .chain(printf_report(*node, source))
                .collect(),
            CALL => brace_format(*node, source),
            _ => Vec::new(),
        })
        .collect()
}

fn percent_format(operation: Node<'_>, source: &[u8]) -> Vec<Report> {
    let is_modulo = operation
        .child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| text(operator, source) == MODULO);
    let format = operation
        .child_by_field_name(LEFT_FIELD)
        .map(unparenthesized)
        .filter(|left| is_modulo && is_string(*left))
        .and_then(|left| text_value(left, source));
    let (Some(format), Some(right)) = (format, operation.child_by_field_name(RIGHT_FIELD)) else {
        return Vec::new();
    };
    let right = unparenthesized(right);
    match percent::summary(&format) {
        Err(PercentError::Unsupported(conversion)) => vec![
            Report::at(PythonRule::F509, operation, ""),
            Report::at(PythonRule::PLE1300, operation, &conversion.to_string()),
        ],
        Err(PercentError::Invalid) => vec![Report::at(PythonRule::F501, operation, "")],
        Ok(summary) => percent_rules(&summary, right, source)
            .into_iter()
            .map(|(rule, subject)| {
                let fix = (rule == PythonRule::F504)
                    .then(|| unused_keys_fix(right, &unused_pairs(&summary, right, source), source));
                Report::at(rule, operation, &subject).with_fix(fix)
            })
            .chain(
                mismatched_types(&format, right, source)
                    .then(|| Report::at(PythonRule::PLE1307, operation, "")),
            )
            .collect(),
    }
}

fn printf_report(operation: Node<'_>, source: &[u8]) -> Option<Report> {
    let is_modulo = operation
        .child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| text(operator, source) == MODULO);
    let left = operation
        .child_by_field_name(LEFT_FIELD)
        .map(unparenthesized)
        .filter(|left| is_modulo && is_string(*left))?;
    let right = operation.child_by_field_name(RIGHT_FIELD).map(unparenthesized)?;
    printf::printf_formatting(operation, left, right, source)
}

fn percent_rules(summary: &PercentSummary, right: Node<'_>, source: &[u8]) -> Vec<(PythonRule, String)> {
    let mut found = Vec::new();
    let is_mapping = matches!(right.kind(), DICTIONARY | DICTIONARY_COMPREHENSION);
    if !summary.keywords.is_empty() && SEQUENCES.contains(&right.kind()) {
        found.push((PythonRule::F502, String::new()));
    }
    if summary.positional > 1 && is_mapping {
        found.push((PythonRule::F503, String::new()));
    }
    if summary.positional == 0 && right.kind() == DICTIONARY {
        found.extend(named_arguments(summary, right, source));
    }
    if summary.positional > 0 && !summary.keywords.is_empty() {
        found.push((PythonRule::F506, String::new()));
    }
    if summary.keywords.is_empty()
        && let Some(given) = substitutions(right)
        && given != summary.positional
    {
        found.push((PythonRule::F507, String::new()));
    }
    if summary.starred && is_mapping {
        found.push((PythonRule::F508, String::new()));
    }
    found
}

fn named_arguments(
    summary: &PercentSummary,
    dictionary: Node<'_>,
    source: &[u8],
) -> Vec<(PythonRule, String)> {
    let items = named(dictionary);
    if items.iter().any(|item| item.kind() != PAIR) {
        return Vec::new();
    }
    let keys: Vec<Option<String>> = items
        .iter()
        .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
        .map(|key| (key.kind() == STRING).then(|| text_value(key, source)).flatten())
        .collect();
    let mut found = Vec::new();
    let unused: Vec<String> = unused_pairs(summary, dictionary, source)
        .into_iter()
        .filter_map(|pair| pair.child_by_field_name(KEY_FIELD))
        .filter_map(|key| text_value(key, source))
        .collect();
    if !unused.is_empty() {
        found.push((PythonRule::F504, unused.join(SEPARATOR)));
    }
    if keys.iter().all(Option::is_some) {
        let missing: Vec<&str> = summary
            .keywords
            .iter()
            .filter(|keyword| !keys.iter().flatten().any(|key| key == *keyword))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            found.push((PythonRule::F505, missing.join(SEPARATOR)));
        }
    }
    found
}

// Los pares con una clave string que el formato no nombra.
fn unused_pairs<'tree>(summary: &PercentSummary, dictionary: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    named(dictionary)
        .into_iter()
        .filter(|pair| {
            pair.child_by_field_name(KEY_FIELD)
                .filter(|key| key.kind() == STRING)
                .and_then(|key| text_value(key, source))
                .is_some_and(|key| !summary.keywords.contains(&key))
        })
        .collect()
}

fn substitutions(right: Node<'_>) -> Option<usize> {
    if right.kind() == TUPLE {
        let elements = named(right);
        return (!elements.iter().any(|element| element.kind() == LIST_SPLAT)).then_some(elements.len());
    }
    let single = is_string(right)
        || SINGLE_VALUES.contains(&right.kind())
        || matches!(right.kind(), DICTIONARY | DICTIONARY_COMPREHENSION | NEGATION)
        || (right.kind() == UNARY
            && named(right)
                .iter()
                .all(|operand| SINGLE_VALUES.contains(&operand.kind())));
    single.then_some(1)
}

fn brace_format(call: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(function) = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)
    else {
        return Vec::new();
    };
    let is_format = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == FORMAT);
    let format = function
        .child_by_field_name(OBJECT_FIELD)
        .map(unparenthesized)
        .filter(|object| is_format && is_string(*object))
        .and_then(|object| text_value(object, source));
    let Some(format) = format else {
        return Vec::new();
    };
    let Some(summary) = braces::summary(&format) else {
        return vec![Report::at(PythonRule::F521, call, "")];
    };
    let invalid_types = braces::invalid_format_types(&format)
        .into_iter()
        .map(|format_type| Report::at(PythonRule::PLE1300, call, &format_type.to_string()));
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD);
    let arguments = arguments.map_or_else(Vec::new, |arguments| {
        if arguments.kind() == GENERATOR {
            vec![arguments]
        } else {
            named(arguments)
        }
    });
    brace_rules(&summary, &arguments, source)
        .into_iter()
        .map(|(rule, subject)| {
            let removed = removable_arguments(rule, &summary, &arguments, source);
            let fix = if removed.is_empty() {
                None
            } else {
                unused_arguments_fix(call, &removed, source)
            };
            Report::at(rule, call, &subject).with_fix(fix)
        })
        .chain(invalid_types)
        .collect()
}

// Lo que sacan los arreglos de F522 y F523. Los posicionales, sólo si son los últimos: sacar uno del
// medio pediría renumerar el formato.
fn removable_arguments<'tree>(
    rule: PythonRule,
    summary: &BraceSummary,
    arguments: &[Node<'tree>],
    source: &[u8],
) -> Vec<Node<'tree>> {
    let (keywords, positional) = split_arguments(arguments);
    if rule == PythonRule::F522 {
        return keywords
            .into_iter()
            .filter(|keyword| {
                keyword
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| !summary.keywords.iter().any(|used| used == text(name, source)))
            })
            .collect();
    }
    if rule != PythonRule::F523 {
        return Vec::new();
    }
    let unused = unused_positions(summary, &positional);
    let contiguous_from_end = unused
        .iter()
        .rev()
        .zip((0..positional.len()).rev())
        .all(|(index, expected)| *index == expected);
    if !contiguous_from_end {
        return Vec::new();
    }
    unused
        .into_iter()
        .filter_map(|index| positional.get(index).copied())
        .collect()
}

fn split_arguments<'tree>(arguments: &[Node<'tree>]) -> (Vec<Node<'tree>>, Vec<Node<'tree>>) {
    arguments
        .iter()
        .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT))
}

fn unused_positions(summary: &BraceSummary, positional: &[Node<'_>]) -> Vec<usize> {
    positional
        .iter()
        .enumerate()
        .filter(|(index, argument)| {
            argument.kind() != LIST_SPLAT
                && !summary.autos.contains(index)
                && !summary.indices.contains(index)
        })
        .map(|(index, _)| index)
        .collect()
}

fn brace_rules(summary: &BraceSummary, arguments: &[Node<'_>], source: &[u8]) -> Vec<(PythonRule, String)> {
    let (keywords, positional) = split_arguments(arguments);
    let names: Vec<&str> = keywords
        .iter()
        .filter_map(|keyword| keyword.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source))
        .collect();
    let spread_keywords = keywords.iter().any(|keyword| keyword.kind() == DICTIONARY_SPLAT);
    let spread_positional = positional.iter().any(|argument| argument.kind() == LIST_SPLAT);
    let mut found = Vec::new();
    let unused_names: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| !summary.keywords.iter().any(|keyword| keyword == name))
        .collect();
    if !spread_keywords && !unused_names.is_empty() {
        found.push((PythonRule::F522, unused_names.join(SEPARATOR)));
    }
    let unused_positions: Vec<String> = unused_positions(summary, &positional)
        .iter()
        .map(ToString::to_string)
        .collect();
    if !unused_positions.is_empty() {
        found.push((PythonRule::F523, unused_positions.join(SEPARATOR)));
    }
    let missing: Vec<String> = summary
        .autos
        .iter()
        .chain(&summary.indices)
        .filter(|index| **index >= positional.len())
        .map(ToString::to_string)
        .chain(
            summary
                .keywords
                .iter()
                .filter(|keyword| !names.contains(&keyword.as_str()))
                .cloned(),
        )
        .collect();
    if !spread_keywords && !spread_positional && !missing.is_empty() {
        found.push((PythonRule::F524, missing.join(SEPARATOR)));
    }
    if !summary.autos.is_empty() && !summary.indices.is_empty() {
        found.push((PythonRule::F525, String::new()));
    }
    if is_explicit_positions_only(summary, &positional, &keywords) {
        found.push((PythonRule::UP030, String::new()));
    }
    found
}

// UP030: `"{0} {1}".format(a, b)`, donde los números sobran.
fn is_explicit_positions_only(
    summary: &BraceSummary,
    positional: &[Node<'_>],
    keywords: &[Node<'_>],
) -> bool {
    let indices = &summary.indices;
    let plain = !summary.has_nested_parts && summary.keywords.is_empty() && summary.autos.is_empty();
    if !plain || indices.is_empty() || (0..indices.len()).any(|index| !indices.contains(&index)) {
        return false;
    }
    let sequential = indices
        .iter()
        .enumerate()
        .all(|(position, index)| position == *index);
    let positional: Vec<&Node<'_>> = positional
        .iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    sequential
        || (keywords.is_empty()
            && positional.len() >= indices.len()
            && positional
                .iter()
                .take(indices.len())
                .all(|argument| argument.kind() != LIST_SPLAT))
}
