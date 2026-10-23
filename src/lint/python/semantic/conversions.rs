use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, CALL, COMMENT, FUNCTION_FIELD, INTERPOLATION, KEYWORD_ARGUMENT, NAME_FIELD, is_f_string,
    named, text,
};
use crate::rules::PythonRule;

const STR: &str = "str";
const CONVERSIONS: [&str; 3] = [STR, "repr", "ascii"];
const OBJECT_KEYWORD: &str = "object";
const TYPE_CONVERSION: &str = "type_conversion";
const DEBUG_MARKER: &str = "=";
const STARRED: [&str; 2] = ["list_splat", "dictionary_splat"];

enum Interpolated<'tree> {
    Convertible(Node<'tree>),
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
                Interpolated::Convertible(call) => reports.push(Report::at(PythonRule::RUF010, call, "")),
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
        Interpolated::Convertible(call)
    }
}
