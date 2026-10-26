use tree_sitter::Node;

use super::Report;
use super::ast::{
    BLOCK, BODY_FIELD, CLASS, COMMENT, EXPRESSION_STATEMENT, FUNCTION, FUTURE_IMPORT, NAME_FIELD, fields,
    is_string, named, text,
};
use super::fix_edits::{delete_between, rewrite_names};
use super::statement_removal::{delete_statement_only, isolation_group};
use crate::fix::Fix;
use crate::rules::PythonRule;

const PASS: &str = "pass_statement";
const ELLIPSIS: &str = "ellipsis";
const ELLIPSIS_TEXT: &str = "...";
const ANNOTATIONS: &str = "annotations";
const HASH: char = '#';
const HASH_BYTE: u8 = b'#';
const TYPE_DIRECTIVE: &str = "type:";
const IGNORE: &str = "ignore";

// Reglas sobre los cuerpos de un stub: PYI009 (`pass` como cuerpo entero), PYI010 (función con algo
// que no es `...`), PYI048 (función con varias sentencias), PYI012 (`pass` en una clase con más
// cosas), PYI044 (`from __future__ import annotations`) y PYI033 (comentarios `# type:`). PYI013
// (`...` en una clase con más cosas) corre en cualquier archivo.
pub fn reports(nodes: &[Node<'_>], source: &[u8], stub: bool) -> Vec<Report> {
    let mut reports = Vec::new();
    for node in nodes.iter().filter(|node| node.is_named()) {
        match node.kind() {
            CLASS => {
                let body = statements(*node);
                if body.len() > 1 {
                    reports.extend(redundant_ellipses(&body, source));
                }
                if stub {
                    reports.extend(class_reports(&body, source));
                }
            }
            FUNCTION if stub => reports.extend(function_reports(*node, source)),
            FUTURE_IMPORT if stub && imports_annotations(*node, source) => {
                reports.push(
                    Report::at(PythonRule::PYI044, *node, "").with_fix(future_annotations_fix(*node, source)),
                );
            }
            COMMENT if stub && is_type_comment(text(*node, source)) => {
                reports.push(Report::at(PythonRule::PYI033, *node, ""));
            }
            _ => {}
        }
    }
    reports
}

fn statements(definition: Node<'_>) -> Vec<Node<'_>> {
    definition
        .child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == BLOCK)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|statement| statement.kind() != COMMENT)
        .collect()
}

fn class_reports(body: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    match body {
        [only] if only.kind() == PASS => vec![ellipsis_report(PythonRule::PYI009, *only, source)],
        [_] | [] => Vec::new(),
        _ => body
            .iter()
            .filter(|statement| statement.kind() == PASS)
            .map(|statement| {
                Report::at(PythonRule::PYI012, *statement, "")
                    .with_fix(Some(delete_statement_only(*statement, source)))
            })
            .collect(),
    }
}

fn function_reports(function: Node<'_>, source: &[u8]) -> Vec<Report> {
    let body = statements(function);
    match body.as_slice() {
        [only] if only.kind() == PASS => vec![ellipsis_report(PythonRule::PYI009, *only, source)],
        [only] if is_ellipsis(*only) || is_docstring(*only) => Vec::new(),
        [only] => vec![ellipsis_report(PythonRule::PYI010, *only, source)],
        [] => Vec::new(),
        _ => function
            .child_by_field_name(NAME_FIELD)
            .map(|name| Report::at(PythonRule::PYI048, name, text(name, source)))
            .into_iter()
            .collect(),
    }
}

fn redundant_ellipses(body: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    body.iter()
        .filter(|statement| is_ellipsis(**statement))
        .map(|statement| {
            Report::at(PythonRule::PYI013, *statement, "")
                .with_fix(Some(ellipsis_removal(*statement, source)))
        })
        .collect()
}

// PYI009 y PYI010: el cuerpo pasa a `...`.
fn ellipsis_report(rule: PythonRule, statement: Node<'_>, source: &[u8]) -> Report {
    Report::at(rule, statement, "").with_fix(Some(Fix::replace(statement, ELLIPSIS_TEXT, source)))
}

// PYI013: con un comentario en la misma línea se borra hasta el comentario; si no, la sentencia.
fn ellipsis_removal(statement: Node<'_>, source: &[u8]) -> Fix {
    let after = source.get(statement.end_byte()..).unwrap_or_default();
    let spaces = after
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    if after.get(spaces) == Some(&HASH_BYTE) {
        let fix = delete_between(
            statement.start_byte(),
            statement.end_byte().saturating_add(spaces),
            source,
        );
        return fix.isolated(isolation_group(statement));
    }
    delete_statement_only(statement, source)
}

// PYI044: se saca `annotations` del import del futuro, o la sentencia entera si era lo único.
fn future_annotations_fix(statement: Node<'_>, source: &[u8]) -> Option<Fix> {
    let names = fields(statement, NAME_FIELD);
    let kept: Vec<&str> = names
        .iter()
        .map(|name| text(*name, source))
        .filter(|name| *name != ANNOTATIONS)
        .collect();
    if kept.is_empty() {
        return Some(delete_statement_only(statement, source).isolated(None));
    }
    rewrite_names(statement, &names, &kept.join(", "), source)
}

fn expression(statement: Node<'_>) -> Option<Node<'_>> {
    (statement.kind() == EXPRESSION_STATEMENT)
        .then(|| match named(statement).as_slice() {
            [only] => Some(*only),
            _ => None,
        })
        .flatten()
}

fn is_ellipsis(statement: Node<'_>) -> bool {
    expression(statement).is_some_and(|value| value.kind() == ELLIPSIS)
}

fn is_docstring(statement: Node<'_>) -> bool {
    expression(statement).is_some_and(is_string)
}

fn imports_annotations(statement: Node<'_>, source: &[u8]) -> bool {
    named(statement)
        .into_iter()
        .any(|name| text(name, source).split_whitespace().next() == Some(ANNOTATIONS))
}

// `# type: X` sin ser `# type: ignore`, con la expresión del tipo antes de cualquier otro `#`.
fn is_type_comment(comment: &str) -> bool {
    let Some(directive) = comment
        .strip_prefix(HASH)
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix(TYPE_DIRECTIVE))
    else {
        return false;
    };
    let annotation = directive.split(HASH).next().unwrap_or_default();
    !annotation.trim().is_empty() && !directive.trim_start().starts_with(IGNORE)
}
