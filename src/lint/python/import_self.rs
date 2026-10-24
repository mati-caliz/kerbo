use std::path::Path;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ALIASED_IMPORT, DOTTED_NAME, IMPORT, IMPORT_FROM, MODULE_NAME_FIELD, NAME_FIELD, fields, named, text,
};
use super::imports::package_root;
use crate::rules::PythonRule;

const SEPARATOR: char = '.';
const RELATIVE_IMPORT: &str = "relative_import";
const IMPORT_PREFIX: &str = "import_prefix";

// PLW0406: el módulo se importa a sí mismo (`import paquete.modulo` adentro de `paquete/modulo.py`).
// Como ruff, sólo dentro de un paquete, y el nombre de un `__init__.py` termina en `__init__`, así que
// ahí nunca coincide.
pub fn reports(nodes: &[Node<'_>], source: &[u8], path: Option<&Path>) -> Vec<Report> {
    let Some(module_path) = path.and_then(module_path) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            IMPORT => found.extend(self_imports(*node, source, &module_path)),
            IMPORT_FROM => found.extend(self_import_from(*node, source, &module_path)),
            _ => {}
        }
    }
    found
}

fn module_path(path: &Path) -> Option<Vec<String>> {
    let root = path.parent().and_then(package_root)?;
    let relative = path.strip_prefix(root.parent()?).ok()?.with_extension("");
    let components: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!components.is_empty()).then_some(components)
}

fn imported_name(name: Node<'_>) -> Option<Node<'_>> {
    match name.kind() {
        DOTTED_NAME => Some(name),
        ALIASED_IMPORT => name.child_by_field_name(NAME_FIELD),
        _ => None,
    }
}

fn self_imports(statement: Node<'_>, source: &[u8], module_path: &[String]) -> Vec<Report> {
    fields(statement, NAME_FIELD)
        .into_iter()
        .filter_map(|name| {
            let imported = text(imported_name(name)?, source);
            imported
                .split(SEPARATOR)
                .eq(module_path.iter().map(String::as_str))
                .then(|| Report::at(PythonRule::PLW0406, name, imported))
        })
        .collect()
}

fn self_import_from(statement: Node<'_>, source: &[u8], module_path: &[String]) -> Option<Report> {
    let module = statement.child_by_field_name(MODULE_NAME_FIELD)?;
    let resolved = resolved_module(module, source, module_path)?;
    let (last, package) = module_path.split_last()?;
    if !resolved.split(SEPARATOR).eq(package.iter().map(String::as_str)) {
        return None;
    }
    fields(statement, NAME_FIELD)
        .into_iter()
        .find(|name| imported_name(*name).is_some_and(|imported| text(imported, source) == last))
        .map(|name| Report::at(PythonRule::PLW0406, name, &format!("{resolved}{SEPARATOR}{last}")))
}

// `resolve_imported_module_path` de ruff: un import relativo sube `nivel` partes desde el módulo.
fn resolved_module(module: Node<'_>, source: &[u8], module_path: &[String]) -> Option<String> {
    if module.kind() != RELATIVE_IMPORT {
        return Some(text(module, source).to_owned());
    }
    let level = named(module)
        .into_iter()
        .find(|part| part.kind() == IMPORT_PREFIX)
        .map_or(0, |prefix| text(prefix, source).chars().count());
    let member = named(module)
        .into_iter()
        .find(|part| part.kind() == DOTTED_NAME)
        .map(|name| text(name, source));
    if level >= module_path.len() {
        return None;
    }
    let mut parts: Vec<&str> = module_path
        .get(..module_path.len().saturating_sub(level))?
        .iter()
        .map(String::as_str)
        .collect();
    parts.extend(member);
    Some(parts.join(&SEPARATOR.to_string()))
}
