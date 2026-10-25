use tree_sitter::Node;

use super::call_fixes::{builtin_available, builtin_fix};
use super::handlers::vanilla_raise;
use super::model::Model;
use super::qualified::qualified_name;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{ATTRIBUTE, COMMENT, IDENTIFIER, TUPLE, named, text, unparenthesized};
use crate::lint::python::fix_edits::pad;
use crate::rules::PythonRule;

// `os_error_alias` de ruff: nombres viejos de `OSError`.
const OS_ERROR_ALIASES: [&str; 8] = [
    "builtins.EnvironmentError",
    "builtins.IOError",
    "builtins.WindowsError",
    "mmap.error",
    "resource.error",
    "select.error",
    "socket.error",
    "os.error",
];
// `timeout_error_alias` de ruff para Python 3.11: nombres viejos de `TimeoutError`.
const TIMEOUT_ERROR_ALIASES: [&str; 2] = ["socket.timeout", "asyncio.TimeoutError"];
const CAUSE_FIELD: &str = "cause";
const OS_ERROR: &str = "OSError";
const TIMEOUT_ERROR: &str = "TimeoutError";
const BUILTINS_PREFIX: &str = "builtins.";
const EXPRESSION_LIST: &str = "expression_list";

// UP024 y UP041 en una llamada (`IOError(...)`): avisan en lo que se llama.
pub fn called_alias_reports(model: &Model<'_>, source: &[u8], function: Node<'_>) -> Vec<Report> {
    alias_rules(model, source, function)
        .map(|rule| Report::at(rule, function, "").with_fix(atom_fix(model, function, rule, source)))
        .collect()
}

// TRY002, UP024 y UP041 en un `raise`: el nombre viejo sin llamar (`raise IOError`).
pub fn raise_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    let mut reports: Vec<Report> = vanilla_raise(model, source, statement)
        .map(|raised| Report::at(PythonRule::TRY002, raised, ""))
        .into_iter()
        .collect();
    let cause = statement.child_by_field_name(CAUSE_FIELD).map(|cause| cause.id());
    let raised = named(statement)
        .into_iter()
        .find(|child| child.kind() != COMMENT && Some(child.id()) != cause)
        .map(unparenthesized)
        .filter(|raised| matches!(raised.kind(), IDENTIFIER | ATTRIBUTE));
    if let Some(raised) = raised {
        reports.extend(
            alias_rules(model, source, raised)
                .map(|rule| Report::at(rule, raised, "").with_fix(atom_fix(model, raised, rule, source))),
        );
    }
    reports
}

// UP024 y UP041 en el tipo de un `except`: el nombre, o la tupla entera si alguno de sus elementos
// es un nombre viejo.
pub fn handler_alias_reports(model: &Model<'_>, source: &[u8], exception_type: Node<'_>) -> Vec<Report> {
    let exception_type = unparenthesized(exception_type);
    match exception_type.kind() {
        IDENTIFIER | ATTRIBUTE => alias_rules(model, source, exception_type)
            .map(|rule| {
                Report::at(rule, exception_type, "").with_fix(atom_fix(model, exception_type, rule, source))
            })
            .collect(),
        TUPLE | EXPRESSION_LIST => {
            let elements = named(exception_type);
            [PythonRule::UP024, PythonRule::UP041]
                .into_iter()
                .filter(|rule| {
                    elements
                        .iter()
                        .any(|element| alias_rules(model, source, *element).any(|found| found == *rule))
                })
                .map(|rule| {
                    let fix = tuple_fix(model, exception_type, rule, source);
                    Report::at(rule, exception_type, "").with_fix(fix)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

const fn builtin_of(rule: PythonRule) -> &'static str {
    if matches!(rule, PythonRule::UP041) {
        TIMEOUT_ERROR
    } else {
        OS_ERROR
    }
}

// `IOError` pasa a `OSError` (o `builtins.OSError` si está tapado).
fn atom_fix(model: &Model<'_>, node: Node<'_>, rule: PythonRule, source: &[u8]) -> Option<Fix> {
    builtin_fix(model, node, builtin_of(rule), source)
}

// En una tupla: sacar los alias, poner el builtin adelante si no estaba y, si queda uno solo, sin
// paréntesis. Sólo si el builtin está libre.
fn tuple_fix(model: &Model<'_>, tuple: Node<'_>, rule: PythonRule, source: &[u8]) -> Option<Fix> {
    let elements = named(tuple);
    let builtin = builtin_of(rule);
    if !builtin_available(model, builtin) {
        return None;
    }
    let qualified_builtin = format!("{BUILTINS_PREFIX}{builtin}");
    let mut remaining: Vec<String> = elements
        .iter()
        .filter(|element| element.kind() != COMMENT)
        .filter(|element| !alias_rules(model, source, **element).any(|found| found == rule))
        .map(|element| text(*element, source).to_owned())
        .collect();
    let has_builtin = elements.iter().any(|element| {
        qualified_name(model, *element, source).as_deref() == Some(qualified_builtin.as_str())
    });
    if !has_builtin {
        remaining.insert(0, builtin.to_owned());
    }
    let content = match remaining.as_slice() {
        [only] => only.clone(),
        _ => format!("({})", remaining.join(", ")),
    };
    Some(Fix::replace(tuple, &pad(content, tuple, source), source))
}

fn alias_rules(model: &Model<'_>, source: &[u8], node: Node<'_>) -> impl Iterator<Item = PythonRule> {
    let name = qualified_name(model, node, source);
    let is_os_error = name
        .as_deref()
        .is_some_and(|name| OS_ERROR_ALIASES.contains(&name));
    let is_timeout_error = name
        .as_deref()
        .is_some_and(|name| TIMEOUT_ERROR_ALIASES.contains(&name));
    [
        is_os_error.then_some(PythonRule::UP024),
        is_timeout_error.then_some(PythonRule::UP041),
    ]
    .into_iter()
    .flatten()
}
