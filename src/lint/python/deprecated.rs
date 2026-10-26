mod tables;

use tree_sitter::Node;

use super::Report;
use super::ast::{ALIASED_IMPORT, IMPORT_FROM, MODULE_NAME_FIELD, NAME_FIELD, fields, text};
use crate::fix::Fix;
use crate::rules::PythonRule;
use tables::{MOVED_MEMBERS, RENAMED_TYPING_MEMBERS};

const TYPING: &str = "typing";
const DOTTED_NAME: &str = "dotted_name";
const COMMA: &str = ",";

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
    let renamed = if module == TYPING {
        members
            .iter()
            .filter(|member| RENAMED_TYPING_MEMBERS.contains(member))
            .count()
    } else {
        0
    };
    MOVED_MEMBERS
        .iter()
        .filter(|(origin, _, moved)| *origin == module && members.iter().any(|member| moved.contains(member)))
        .map(|(_, target, moved)| {
            Report::at(PythonRule::UP035, statement, "")
                .with_fix(moved_import_fix(statement, target, moved, source))
        })
        .chain(std::iter::repeat_with(|| Report::at(PythonRule::UP035, statement, "")).take(renamed))
        .collect()
}

// El arreglo de UP035, como el `ImportReplacer` de ruff: si se mudan todos, la sentencia pasa a
// importar del módulo nuevo; si no, se sacan de la lista (con su coma) y se agrega abajo, con la
// misma sangría, `from nuevo import ...`. Sin arreglo si la sentencia no empieza su línea.
fn moved_import_fix(statement: Node<'_>, target: &str, moved: &[&str], source: &[u8]) -> Option<Fix> {
    let names = fields(statement, NAME_FIELD);
    let member_of = |name: Node<'_>| {
        let member = if name.kind() == ALIASED_IMPORT {
            name.child_by_field_name(NAME_FIELD).unwrap_or(name)
        } else {
            name
        };
        text(member, source)
    };
    let matched: Vec<Node<'_>> = names
        .iter()
        .copied()
        .filter(|name| moved.contains(&member_of(*name)))
        .collect();
    let listed: Vec<String> = matched
        .iter()
        .map(|name| {
            text(*name, source)
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ")
        })
        .collect();
    let new_import = format!("from {target} import {}", listed.join(", "));
    if matched.len() == names.len() {
        return Some(Fix::replace(statement, &new_import, source));
    }
    let line_start = source
        .get(..statement.start_byte())?
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position.saturating_add(1));
    let indentation = source.get(line_start..statement.start_byte())?;
    if !indentation.iter().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }
    let commas: Vec<Node<'_>> = (0..statement.child_count())
        .filter_map(|index| statement.child(index))
        .filter(|child| child.kind() == COMMA)
        .collect();
    let mut removed: Vec<(usize, usize)> = Vec::new();
    let mut is_first = true;
    for (index, name) in names.iter().enumerate() {
        if !matched.iter().any(|member| member.id() == name.id()) {
            is_first = false;
            continue;
        }
        let range = if is_first {
            (
                name.start_byte(),
                names.get(index.saturating_add(1))?.start_byte(),
            )
        } else {
            (commas.get(index.checked_sub(1)?)?.start_byte(), name.end_byte())
        };
        removed.push(range);
    }
    let mut kept = String::new();
    let mut cursor = statement.start_byte();
    for (start, end) in removed {
        kept.push_str(&String::from_utf8_lossy(source.get(cursor..start)?));
        cursor = end;
    }
    kept.push_str(&String::from_utf8_lossy(
        source.get(cursor..statement.end_byte())?,
    ));
    let replacement = format!("{kept}\n{}{new_import}", String::from_utf8_lossy(indentation));
    Some(Fix::replace(statement, &replacement, source))
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
