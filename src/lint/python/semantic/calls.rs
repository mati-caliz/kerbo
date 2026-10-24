use tree_sitter::Node;

use super::aliases::called_alias_reports;
use super::builtin_calls::builtin_call_reports;
use super::cast_calls::cast_call_reports;
use super::collection_calls::collection_call_reports;
use super::decimals::{decimal_from_float_literal, verbose_decimal_constructor};
use super::format_calls::format_call_report;
use super::idioms::idiom_reports;
use super::integer_calls::integer_call_reports;
use super::misc_calls::misc_call_reports;
use super::model::Model;
use super::qualified::{is_builtin_named, qualified_name};
use super::resources::unmanaged_open;
use super::supers::is_super_with_parameters;
use super::suspicious_calls::suspicious_call_reports;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, EXPRESSION_STATEMENT, FUNCTION_FIELD,
    KEYWORD_ARGUMENT, LIST, OBJECT_FIELD, VALUE_FIELD, is_identifier, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const RANGE: &str = "range";
const IO_OPEN: &str = "io.open";
const GETATTR: &str = "getattr";
const SETATTR: &str = "setattr";
const DEBUG_NAME: &str = "__debug__";
const PRIVATE_PREFIX: &str = "__";
const INTEGER: &str = "integer";
const LIST_SPLAT: &str = "list_splat";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const LIST_COMPREHENSION: &str = "list_comprehension";
const POP: &str = "pop";
const SEQUENCE_BUILDERS: [&str; 2] = ["tuple", "list"];
const SUBSCRIPT_FIELD: &str = "subscript";
const ARGUMENTS_KIND: &str = "argument_list";
const RADIX_PREFIXES: [&str; 6] = ["0x", "0X", "0o", "0O", "0b", "0B"];
const ZERO_DIGITS: [char; 2] = ['0', '_'];
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];

pub struct CallArguments<'tree> {
    pub positional: Vec<Node<'tree>>,
    pub keywords: Vec<Node<'tree>>,
}

pub fn call_arguments(call: Node<'_>) -> CallArguments<'_> {
    let (keywords, positional) = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT));
    CallArguments { positional, keywords }
}

// Los avisos sobre una llamada que no dependen del recorrido: PIE808, B009, B010, RUF015, SIM115 y
// UP008.
pub fn call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let mut reports: Vec<Report> = call_rule(model, source, call)
        .map(|(rule, at)| Report::at(rule, at, ""))
        .into_iter()
        .collect();
    reports
        .extend(unmanaged_open(model, source, call).map(|opener| Report::at(PythonRule::SIM115, opener, "")));
    if is_super_with_parameters(model, source, call)
        && let Some(arguments) = call.child_by_field_name(ARGUMENTS_FIELD)
    {
        reports.push(Report::at(PythonRule::UP008, arguments, ""));
    }
    reports.extend(idiom_reports(model, source, call));
    reports.extend(format_call_report(model, source, call));
    reports.extend(verbose_decimal_constructor(model, source, call));
    reports.extend(decimal_from_float_literal(model, source, call));
    reports.extend(collection_call_reports(model, source, call));
    reports.extend(suspicious_call_reports(model, source, call));
    reports.extend(builtin_call_reports(model, source, call));
    reports.extend(integer_call_reports(model, source, call));
    reports.extend(cast_call_reports(model, source, call));
    reports.extend(misc_call_reports(model, source, call));
    if let Some(function) = call.child_by_field_name(FUNCTION_FIELD) {
        reports.extend(called_alias_reports(model, source, function));
    }
    if call
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|function| qualified_name(model, function, source))
        .is_some_and(|name| name == IO_OPEN)
    {
        reports.push(Report::at(PythonRule::UP020, call, ""));
    }
    reports
}

// PIE808 (`range(0, n)`), B009 (`getattr(x, "a")`), B010 (`setattr(x, "a", v)` como sentencia) y
// RUF015 (`list(x)[0]` y `list(x).pop(0)`), con dónde avisar.
fn call_rule<'tree>(
    model: &Model<'_>,
    source: &[u8],
    call: Node<'tree>,
) -> Option<(PythonRule, Node<'tree>)> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let arguments = call_arguments(call);
    let is_builtin = |name: &str| is_builtin_named(model, function, source, name);
    match (arguments.positional.as_slice(), arguments.keywords.is_empty()) {
        ([start, _], true) if is_zero(*start, source) && is_builtin(RANGE) => {
            Some((PythonRule::PIE808, *start))
        }
        ([object, name], _) if is_constant_attribute(*object, *name, source) && is_builtin(GETATTR) => {
            Some((PythonRule::B009, call))
        }
        ([object, name, _], _)
            if is_constant_attribute(*object, *name, source)
                && text_value(unparenthesized(*name), source).as_deref() != Some(DEBUG_NAME)
                && is_statement(call)
                && is_builtin(SETATTR) =>
        {
            Some((PythonRule::B010, call))
        }
        ([index], true) if is_zero(*index, source) && is_first_pop(model, source, function) => {
            Some((PythonRule::RUF015, call))
        }
        _ => None,
    }
}

// RUF015 sobre `list(x)[0]`.
pub fn is_first_element_subscript(model: &Model<'_>, source: &[u8], subscript: Node<'_>) -> bool {
    let mut cursor = subscript.walk();
    let slices: Vec<Node<'_>> = subscript
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    matches!(slices.as_slice(), [index] if is_zero(*index, source))
        && subscript
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| is_iteration_target(model, source, value))
}

fn is_first_pop(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == POP)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| is_iteration_target(model, source, object))
}

// `match_iteration_target`: `list(x)`, `tuple(x)`, `[... for ...]` o `[*x]`.
fn is_iteration_target(model: &Model<'_>, source: &[u8], value: Node<'_>) -> bool {
    let value = unparenthesized(value);
    match value.kind() {
        CALL => {
            let builder = value.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
                SEQUENCE_BUILDERS
                    .iter()
                    .any(|name| is_builtin_named(model, function, source, name))
            });
            let single = value
                .child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| {
                    arguments.kind() != ARGUMENTS_KIND || call_arguments(value).positional.len() == 1
                });
            builder && single
        }
        LIST_COMPREHENSION => true,
        LIST => matches!(named(value).as_slice(), [element] if element.kind() == LIST_SPLAT),
        _ => false,
    }
}

fn is_constant_attribute(object: Node<'_>, name: Node<'_>, source: &[u8]) -> bool {
    object.kind() != LIST_SPLAT
        && text_value(unparenthesized(name), source)
            .is_some_and(|name| is_identifier(&name) && !is_mangled_private(&name))
}

fn is_mangled_private(name: &str) -> bool {
    name.starts_with(PRIVATE_PREFIX) && !name.ends_with(PRIVATE_PREFIX)
}

fn is_statement(call: Node<'_>) -> bool {
    std::iter::successors(call.parent(), Node::parent)
        .find(|ancestor| unparenthesized(*ancestor).id() != call.id())
        .is_some_and(|holder| holder.kind() == EXPRESSION_STATEMENT && named(holder).len() == 1)
}

fn is_zero(number: Node<'_>, source: &[u8]) -> bool {
    let number = unparenthesized(number);
    let literal = text(number, source);
    let digits = RADIX_PREFIXES
        .iter()
        .find_map(|prefix| literal.strip_prefix(prefix))
        .unwrap_or(literal);
    number.kind() == INTEGER
        && !literal.ends_with(IMAGINARY_SUFFIXES)
        && !digits.is_empty()
        && digits.chars().all(|digit| ZERO_DIGITS.contains(&digit))
}
