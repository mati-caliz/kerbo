use tree_sitter::Node;

use super::Report;
use super::ast::{
    ALIASED_IMPORT, ASSIGNMENT, ATTRIBUTE, COMMENT, DOTTED_NAME, EXPRESSION_STATEMENT, IDENTIFIER, IMPORT,
    IMPORT_FROM, LEFT_FIELD, MODULE_NAME_FIELD, NAME_FIELD, OBJECT_FIELD, RIGHT_FIELD, TYPE_FIELD, fields,
    named, text,
};
use super::statement_removal::delete_statement;
use crate::fix::Fix;
use crate::rules::PythonRule;

const METACLASS: &str = "__metaclass__";
const TYPE: &str = "type";
const C_ELEMENT_TREE: &str = "xml.etree.cElementTree";
const ELEMENT_TREE_PACKAGE: &str = "xml.etree";
const C_ELEMENT_TREE_MEMBER: &str = "cElementTree";
const ELEMENT_TREE_MEMBER: &str = "ElementTree";
const UNITTEST_MOCK: &str = "unittest.mock";
const ALIAS_FIELD: &str = "alias";
const MOCK: &str = "mock";
const MOCK_MOCK: &str = "mock.mock";

// UP001 (`__metaclass__ = type`), UP023 (`xml.etree.cElementTree`) y UP026 (el paquete `mock` en vez
// de `unittest.mock`).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let imports_mock = nodes.iter().any(|node| imports_module(*node, source, MOCK));
    let mut reports = Vec::new();
    for node in nodes {
        match node.kind() {
            EXPRESSION_STATEMENT if is_useless_metaclass(*node, source) => {
                reports.push(
                    Report::at(PythonRule::UP001, *node, "").with_fix(Some(delete_statement(*node, source))),
                );
            }
            IMPORT => reports.extend(import_reports(*node, source)),
            IMPORT_FROM => reports.extend(import_from_reports(*node, source)),
            ATTRIBUTE if imports_mock => reports.extend(
                node.child_by_field_name(OBJECT_FIELD)
                    .filter(|object| object.kind() == ATTRIBUTE && text(*object, source) == MOCK_MOCK)
                    .map(|object| {
                        Report::at(PythonRule::UP026, object, "")
                            .with_fix(Some(Fix::replace(object, MOCK, source)))
                    }),
            ),
            _ => {}
        }
    }
    reports
}

fn is_useless_metaclass(statement: Node<'_>, source: &[u8]) -> bool {
    let Some(assignment) = named(statement)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
        .filter(|child| child.kind() == ASSIGNMENT && child.child_by_field_name(TYPE_FIELD).is_none())
    else {
        return false;
    };
    let is_name = |field: &str, wanted: &str| {
        assignment
            .child_by_field_name(field)
            .is_some_and(|side| side.kind() == IDENTIFIER && text(side, source) == wanted)
    };
    is_name(LEFT_FIELD, METACLASS) && is_name(RIGHT_FIELD, TYPE)
}

// El nombre importado y si tiene `as`.
// UP023: el primer `cElementTree` pasa a `ElementTree`.
fn element_tree_fix(node: Node<'_>, source: &[u8]) -> Fix {
    let replacement = text(node, source).replacen(C_ELEMENT_TREE_MEMBER, ELEMENT_TREE_MEMBER, 1);
    Fix::replace(node, &replacement, source)
}

// UP026 para `import mock` (o `mock.mock`), sólo cuando es el único nombre de la sentencia: pasa a
// `from unittest import mock`, con el alias si lo tenía.
fn mock_import_fix(statement: Node<'_>, name: Node<'_>, source: &[u8]) -> Option<Fix> {
    if fields(statement, NAME_FIELD).len() != 1 {
        return None;
    }
    let alias = (name.kind() == ALIASED_IMPORT)
        .then(|| name.child_by_field_name(ALIAS_FIELD))
        .flatten()
        .map(|alias| format!(" as {}", text(alias, source)))
        .unwrap_or_default();
    Some(Fix::replace(
        statement,
        &format!("from unittest import mock{alias}"),
        source,
    ))
}

fn imported(name: Node<'_>, source: &[u8]) -> Option<(String, bool)> {
    match name.kind() {
        DOTTED_NAME => Some((text(name, source).to_owned(), false)),
        ALIASED_IMPORT => name
            .child_by_field_name(NAME_FIELD)
            .map(|original| (text(original, source).to_owned(), true)),
        _ => None,
    }
}

fn imports_module(node: Node<'_>, source: &[u8], module: &str) -> bool {
    match node.kind() {
        IMPORT => fields(node, NAME_FIELD).into_iter().any(|name| {
            imported(name, source).is_some_and(|(imported, _)| {
                imported == module || imported.starts_with(&format!("{module}."))
            })
        }),
        IMPORT_FROM => module_name(node, source) == Some(module),
        _ => false,
    }
}

fn import_reports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    fields(statement, NAME_FIELD)
        .into_iter()
        .filter_map(|name| {
            let (module, aliased) = imported(name, source)?;
            if module == C_ELEMENT_TREE && aliased {
                Some(Report::at(PythonRule::UP023, name, "").with_fix(Some(element_tree_fix(name, source))))
            } else if module == MOCK || module == MOCK_MOCK {
                Some(
                    Report::at(PythonRule::UP026, name, "")
                        .with_fix(mock_import_fix(statement, name, source)),
                )
            } else {
                None
            }
        })
        .collect()
}

fn import_from_reports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    match module_name(statement, source) {
        Some(C_ELEMENT_TREE) => {
            vec![
                Report::at(PythonRule::UP023, statement, "")
                    .with_fix(Some(element_tree_fix(statement, source))),
            ]
        }
        Some(MOCK) => {
            let fix = statement
                .child_by_field_name(MODULE_NAME_FIELD)
                .map(|module| Fix::replace(module, UNITTEST_MOCK, source));
            vec![Report::at(PythonRule::UP026, statement, "").with_fix(fix)]
        }
        Some(ELEMENT_TREE_PACKAGE) => fields(statement, NAME_FIELD)
            .into_iter()
            .filter(|name| {
                imported(*name, source)
                    .is_some_and(|(member, aliased)| aliased && member == C_ELEMENT_TREE_MEMBER)
            })
            .map(|name| Report::at(PythonRule::UP023, name, ""))
            .collect(),
        _ => Vec::new(),
    }
}

// El módulo de un `from x import ...` absoluto; `None` si es relativo.
fn module_name<'source>(statement: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    statement
        .child_by_field_name(MODULE_NAME_FIELD)
        .filter(|module| module.kind() == DOTTED_NAME)
        .map(|module| text(module, source))
}
