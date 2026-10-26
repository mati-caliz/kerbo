use std::collections::BTreeMap;

use tree_sitter::Node;

use super::super::Visitor;
use super::super::model::{Binding, BindingId, EXPLICIT_EXPORT, GLOBAL, NONLOCAL};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{ALIASED_IMPORT, NAME_FIELD, fields, text};
use crate::lint::python::fix_edits::rewrite_names;
use crate::lint::python::statement_removal::{delete_statement, isolation_group};
use crate::rules::PythonRule;

const SEPARATOR: &str = ", ";
const LINE_FEED: u8 = b'\n';
const ENTRY_TRAILER: &[u8] = b" \t,\r";

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

fn entry_of(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == ALIASED_IMPORT)
        .unwrap_or(node)
}

fn removal(statement: Node<'_>, unused: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let entries = fields(statement, NAME_FIELD);
    let kept: Vec<Node<'_>> = entries
        .iter()
        .copied()
        .filter(|entry| !unused.iter().any(|removed| removed.id() == entry.id()))
        .collect();
    if kept.is_empty() {
        return Some(delete_statement(statement, source));
    }
    let fix = if statement.start_position().row == statement.end_position().row {
        rewrite_line(statement, &entries, &kept, source)
    } else {
        remove_lines(&entries, unused, source)
    };
    fix.map(|fix| fix.isolated(isolation_group(statement)))
}

fn rewrite_line(statement: Node<'_>, entries: &[Node<'_>], kept: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let kept_text: Vec<&str> = kept.iter().map(|entry| text(*entry, source)).collect();
    rewrite_names(statement, entries, &kept_text.join(SEPARATOR), source)
}

fn remove_lines(entries: &[Node<'_>], unused: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let mut edits = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| unused.iter().any(|removed| removed.id() == entry.id()))
    {
        let row = entry.start_position().row;
        let shared = entries
            .iter()
            .any(|other| other.id() != entry.id() && other.start_position().row == row);
        let line_end = source
            .get(entry.end_byte()..)
            .and_then(|rest| rest.iter().position(|byte| *byte == LINE_FEED))
            .map(|offset| entry.end_byte().saturating_add(offset))?;
        let trailer = source.get(entry.end_byte()..line_end).unwrap_or_default();
        let line_start = entry.start_byte().saturating_sub(entry.start_position().column);
        let leader = source.get(line_start..entry.start_byte()).unwrap_or_default();
        let alone = leader.iter().all(u8::is_ascii_whitespace)
            && trailer.iter().all(|byte| ENTRY_TRAILER.contains(byte));
        if shared || !alone {
            return None;
        }
        edits.push(TextEdit {
            start: TextPoint {
                line: row.saturating_add(1),
                column: 1,
            },
            end: TextPoint {
                line: row.saturating_add(2),
                column: 1,
            },
            replacement: String::new(),
        });
    }
    Some(Fix::from_edits(source, edits))
}
