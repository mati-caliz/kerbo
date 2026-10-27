use std::collections::BTreeMap;

use tree_sitter::Node;

use super::super::Visitor;
use super::super::model::{Binding, BindingId, EXPLICIT_EXPORT, GLOBAL, NONLOCAL};
use super::import_rebuild::rebuilt_import;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{ALIASED_IMPORT, NAME_FIELD, fields};
use crate::lint::python::statement_removal::{delete_statement, isolation_group};
use crate::rules::PythonRule;

pub fn unused_imports(visitor: &Visitor<'_>, latest: &[(&str, BindingId)]) -> Vec<Report> {
    let unused: Vec<&Binding<'_>> = latest
        .iter()
        .filter_map(|(_, id)| visitor.model.binding(*id))
        .filter(|binding| {
            binding.kind.is_import()
                && !binding.is_used()
                && !binding.has(EXPLICIT_EXPORT | GLOBAL | NONLOCAL)
        })
        .collect();
    let mut by_statement: BTreeMap<usize, Vec<Node<'_>>> = BTreeMap::new();
    for binding in &unused {
        if let (Some(statement), Some(node)) = (binding.site.statement, binding.site.node) {
            by_statement
                .entry(statement.id())
                .or_default()
                .push(entry_of(node));
        }
    }
    unused
        .into_iter()
        .filter_map(|binding| {
            let statement = binding.site.statement?;
            let fix = if visitor.file.package_init {
                None
            } else {
                by_statement
                    .get(&statement.id())
                    .and_then(|entries| removal(statement, entries, visitor.source))
            };
            Some(
                Report::at_point(PythonRule::F401, binding.site.at, &binding.qualified)
                    .with_fix(fix)
                    .with_statement_row(statement.start_position().row),
            )
        })
        .collect()
}

pub fn entry_of(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == ALIASED_IMPORT)
        .unwrap_or(node)
}

// El `remove_unused_imports` de ruff: saca esos nombres del import, o la sentencia entera.
pub fn removal(statement: Node<'_>, unused: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let entries = fields(statement, NAME_FIELD);
    let kept: Vec<Node<'_>> = entries
        .iter()
        .copied()
        .filter(|entry| !unused.iter().any(|removed| removed.id() == entry.id()))
        .collect();
    if kept.is_empty() {
        return Some(delete_statement(statement, source));
    }
    let content = rebuilt_import(statement, &kept, source)?;
    Some(Fix::replace(statement, &content, source).isolated(isolation_group(statement)))
}
