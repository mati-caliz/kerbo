use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, CALL, COMMENT, FUNCTION_FIELD, INTERPOLATION, KEYWORD_ARGUMENT, NAME_FIELD, VALUE_FIELD,
    is_f_string, named, text,
};
use crate::lint::python::fix_edits::has_comment_within;
use crate::rules::PythonRule;

const STR: &str = "str";
const CONVERSIONS: [&str; 3] = [STR, "repr", "ascii"];
const OBJECT_KEYWORD: &str = "object";
const TYPE_CONVERSION: &str = "type_conversion";
const DEBUG_MARKER: &str = "=";
const STARRED: [&str; 2] = ["list_splat", "dictionary_splat"];
const FLAGS: [&str; 3] = ["s", "r", "a"];
const STR_FLAG: &str = "s";
const GENERATOR: &str = "generator_expression";
const LOOSE_BINDING: [&str; 3] = ["lambda", "named_expression", "yield"];

enum Interpolated<'tree> {
    Convertible(Convertible<'tree>),
    Plain,
    Stop,
}

// RUF010: `f"{str(x)}"`, `f"{repr(x)}"` o `f"{ascii(x)}"`, que se escriben `{x!s}`, `{x!r}` y `{x!a}`.
pub fn conversion_reports(model: &Model<'_>, source: &[u8], parts: &[Node<'_>]) -> Vec<Report> {
    let mut reports = Vec::new();
    for part in parts.iter().filter(|part| is_f_string(**part, source)) {
        for interpolation in named(*part)
            .into_iter()
            .filter(|child| child.kind() == INTERPOLATION)
        {
            match explicit_conversion(model, source, interpolation) {
                Interpolated::Convertible(found) => {
                    let fix = conversion_flag_fix(*part, interpolation, &found, source);
                    reports.push(Report::at(PythonRule::RUF010, found.call, "").with_fix(Some(fix)));
                }
                Interpolated::Plain => {}
                Interpolated::Stop => break,
            }
        }
    }
    reports
}

fn explicit_conversion<'tree>(
    model: &Model<'_>,
    source: &[u8],
    interpolation: Node<'tree>,
) -> Interpolated<'tree> {
    let mut cursor = interpolation.walk();
    let children: Vec<Node<'tree>> = interpolation.children(&mut cursor).collect();
    if children.iter().any(|child| child.kind() == TYPE_CONVERSION) {
        return Interpolated::Plain;
    }
    let Some(call) = named(interpolation)
        .first()
        .copied()
        .filter(|expression| expression.kind() == CALL)
    else {
        return Interpolated::Plain;
    };
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Interpolated::Plain;
    };
    let Some(conversion) = CONVERSIONS
        .into_iter()
        .find(|conversion| is_builtin_named(model, function, source, conversion))
    else {
        return Interpolated::Plain;
    };
    let arguments: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .collect();
    let argument = match arguments.as_slice() {
        [keyword] if keyword.kind() == KEYWORD_ARGUMENT => {
            let is_object = conversion == STR
                && keyword
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|name| text(name, source) == OBJECT_KEYWORD);
            if !is_object {
                return Interpolated::Plain;
            }
            *keyword
        }
        [argument] if conversion == STR && STARRED.contains(&argument.kind()) => return Interpolated::Plain,
        [argument] => *argument,
        _ => return Interpolated::Plain,
    };
    let debug = children.iter().any(|child| child.kind() == DEBUG_MARKER);
    if STARRED.contains(&argument.kind()) || debug {
        Interpolated::Stop
    } else {
        let value = if argument.kind() == KEYWORD_ARGUMENT {
            argument.child_by_field_name(VALUE_FIELD).unwrap_or(argument)
        } else {
            argument
        };
        Interpolated::Convertible(Convertible {
            call,
            argument: value,
            flag: conversion_flag(conversion),
        })
    }
}

struct Convertible<'tree> {
    call: Node<'tree>,
    argument: Node<'tree>,
    flag: &'static str,
}

fn conversion_flag(conversion: &str) -> &'static str {
    CONVERSIONS
        .iter()
        .zip(FLAGS)
        .find(|(name, _)| **name == conversion)
        .map_or(STR_FLAG, |(_, flag)| flag)
}

// Como el arreglo de ruff con libcst: reescribe la parte entera del f-string con el argumento en lugar
// de la llamada y la conversión antes del formato o la llave que cierra. Entre paréntesis si el
// argumento liga menos que una lambda, y con un espacio después de la llave si empieza con otra.
// Inseguro si se pierde un comentario de la llamada.
fn conversion_flag_fix(
    part: Node<'_>,
    interpolation: Node<'_>,
    found: &Convertible<'_>,
    source: &[u8],
) -> Fix {
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let call = found.call;
    let written = text(found.argument, source);
    let needs_parentheses = LOOSE_BINDING.contains(&found.argument.kind())
        || (found.argument.kind() == GENERATOR && !written.starts_with('('));
    let mut argument = if needs_parentheses {
        format!("({written})")
    } else {
        written.to_owned()
    };
    let after_call = source
        .get(call.end_byte()..interpolation.end_byte())
        .unwrap_or_default();
    let whitespace_after = after_call
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let flag_at = call.end_byte().saturating_add(whitespace_after);
    let mut expression_start = call.start_byte();
    if written.starts_with('{') {
        expression_start = interpolation.start_byte().saturating_add(1);
        argument.insert(0, ' ');
    }
    let replacement = format!(
        "{}{argument}{}!{}{}",
        slice(part.start_byte(), expression_start),
        slice(call.end_byte(), flag_at),
        found.flag,
        slice(flag_at, part.end_byte()),
    );
    let fix = Fix::replace(part, &replacement, source);
    let deletes_comment = has_comment_within(call, call.start_byte(), found.argument.start_byte())
        || has_comment_within(call, found.argument.end_byte(), call.end_byte());
    if deletes_comment { fix.marked_unsafe() } else { fix }
}
