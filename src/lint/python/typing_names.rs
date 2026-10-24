use tree_sitter::Node;

use super::Report;
use super::ast::{IDENTIFIER, LEFT_FIELD, NAME_FIELD, named, text};
use crate::rules::PythonRule;

const IMPORT_FROM: &str = "import_from_statement";
const TYPE_ALIAS_STATEMENT: &str = "type_alias_statement";
const ALIASED_IMPORT: &str = "aliased_import";
const MODULE_NAME_FIELD: &str = "module_name";
const ALIAS_FIELD: &str = "alias";
const GENERIC_TYPE: &str = "generic_type";
const COLLECTIONS_ABC: &str = "collections.abc";
const SET: &str = "Set";
const ABSTRACT_SET: &str = "AbstractSet";
const BYTE_STRING: &str = "ByteString";
const BYTE_STRING_MODULES: [&str; 3] = ["typing", "typing_extensions", COLLECTIONS_ABC];
const PRIVATE_PREFIX: char = '_';

// PYI042 (un `type` en snake_case; el `TypeAlias` lo mira el modelo semántico), PYI025 (`from collections.abc import Set` sin llamarlo
// `AbstractSet`) y PYI057 (importar `ByteString`).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            TYPE_ALIAS_STATEMENT => found.extend(snake_case_statement(*node, source)),
            IMPORT_FROM => found.extend(import_reports(*node, source)),
            _ => {}
        }
    }
    found
}

fn snake_case_statement(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let target = named(statement.child_by_field_name(LEFT_FIELD)?)
        .into_iter()
        .next()?;
    let name = if target.kind() == GENERIC_TYPE {
        named(target).into_iter().next()?
    } else {
        target
    };
    (name.kind() == IDENTIFIER && is_snake_case(text(name, source)))
        .then(|| Report::at(PythonRule::PYI042, name, ""))
}

// `is_snake_case_type_alias` de ruff: empieza en minúscula o dígito, quizás después de un `_`.
pub fn is_snake_case(name: &str) -> bool {
    let mut letters = name.chars();
    match (letters.next(), letters.next()) {
        (Some(PRIVATE_PREFIX), Some(second)) => second.is_ascii_lowercase() || second.is_ascii_digit(),
        (Some(first), _) => first.is_ascii_lowercase() || first.is_ascii_digit(),
        _ => false,
    }
}

fn import_reports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(module) = statement
        .child_by_field_name(MODULE_NAME_FIELD)
        .map(|module| text(module, source))
    else {
        return Vec::new();
    };
    let mut cursor = statement.walk();
    statement
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .filter_map(|imported| {
            let (name, alias) = if imported.kind() == ALIASED_IMPORT {
                (
                    imported.child_by_field_name(NAME_FIELD)?,
                    imported
                        .child_by_field_name(ALIAS_FIELD)
                        .map(|alias| text(alias, source)),
                )
            } else {
                (imported, None)
            };
            let member = text(name, source);
            if member == SET && module == COLLECTIONS_ABC {
                let bound = alias.unwrap_or(member);
                return (bound != ABSTRACT_SET && !bound.starts_with(PRIVATE_PREFIX))
                    .then(|| Report::at(PythonRule::PYI025, imported, ""));
            }
            (member == BYTE_STRING && BYTE_STRING_MODULES.contains(&module))
                .then(|| Report::at(PythonRule::PYI057, name, ""))
        })
        .collect()
}
