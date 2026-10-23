mod tables;

use tree_sitter::Node;

use super::Report;
use super::ast::{ALIASED_IMPORT, IMPORT_FROM, MODULE_NAME_FIELD, NAME_FIELD, text};
use crate::rules::PythonRule;
use tables::{MOVED_MEMBERS, RENAMED_TYPING_MEMBERS};

const TYPING: &str = "typing";
const DOTTED_NAME: &str = "dotted_name";

// UP035: `from typing import Callable` y compañía, importados de un módulo del que se mudaron. Un
// aviso por cada destino, y uno por cada miembro de `typing` que además cambió de nombre.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == IMPORT_FROM)
        .flat_map(|statement| deprecated_imports(*statement, source))
        .collect()
}

fn deprecated_imports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(module) = statement
        .child_by_field_name(MODULE_NAME_FIELD)
        .filter(|module| module.kind() == DOTTED_NAME)
        .map(|module| text(module, source))
    else {
        return Vec::new();
    };
    let members = imported_members(statement, source);
    let moved = MOVED_MEMBERS
        .iter()
        .filter(|(origin, _, moved)| *origin == module && members.iter().any(|member| moved.contains(member)))
        .count();
    let renamed = if module == TYPING {
        members
            .iter()
            .filter(|member| RENAMED_TYPING_MEMBERS.contains(member))
            .count()
    } else {
        0
    };
    std::iter::repeat_with(|| Report::at(PythonRule::UP035, statement, ""))
        .take(moved.saturating_add(renamed))
        .collect()
}

fn imported_members<'source>(statement: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    let mut cursor = statement.walk();
    statement
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .filter_map(|name| {
            if name.kind() == ALIASED_IMPORT {
                name.child_by_field_name(NAME_FIELD)
            } else {
                Some(name)
            }
        })
        .map(|name| text(name, source))
        .collect()
}
