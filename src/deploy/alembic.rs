mod calls;
mod dialects;

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::Violation;
use crate::rules::{MigrationRule, RuleId};
use crate::syntax::{named_children, one_based};
use calls::{Call, string_value};

const ALEMBIC_MARKERS: &[&str] = &["from alembic import", "import alembic"];
const UPGRADE: &str = "upgrade";
const OPERATIONS: &str = "op";
const BATCH: &str = "batch_alter_table";
const EXECUTE: &str = "execute";

#[derive(Debug, Default)]
struct Upgrade {
    batches: HashMap<String, String>,
    created_tables: HashSet<String>,
    violations: Vec<Violation>,
}

pub fn violations(root: Node<'_>, source: &[u8]) -> Vec<Violation> {
    let text = String::from_utf8_lossy(source);
    if !ALEMBIC_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Vec::new();
    }
    let mut upgrade = Upgrade::default();
    let functions = named_children(root)
        .into_iter()
        .filter(|node| node.kind() == "function_definition")
        .filter(|node| {
            node.child_by_field_name("name")
                .and_then(|name| name.utf8_text(source).ok())
                == Some(UPGRADE)
        });
    for function in functions {
        let mut pending = vec![function];
        while let Some(node) = pending.pop() {
            upgrade.visit(node, source);
            pending.extend(dialects::postgres_children(node, source).into_iter().rev());
        }
    }
    upgrade.violations
}

impl Upgrade {
    fn visit(&mut self, node: Node<'_>, source: &[u8]) {
        if node.kind() == "with_item" {
            self.remember_batch(node, source);
        } else if let Some(call) = Call::parse(node, source) {
            self.call(&call, source);
        }
    }

    fn remember_batch(&mut self, item: Node<'_>, source: &[u8]) {
        let Some(pattern) = item
            .child_by_field_name("value")
            .filter(|value| value.kind() == "as_pattern")
        else {
            return;
        };
        let call = named_children(pattern)
            .into_iter()
            .find_map(|child| Call::parse(child, source))
            .filter(|call| call.receiver == OPERATIONS && call.method == BATCH);
        let alias = pattern
            .child_by_field_name("alias")
            .and_then(|alias| alias.utf8_text(source).ok());
        if let (Some(call), Some(alias)) = (call, alias) {
            let table = call.positional(0).and_then(|node| string_value(node, source));
            self.batches.insert(alias.to_owned(), table.unwrap_or_default());
        }
    }

    fn report(&mut self, rule: MigrationRule, call: &Call<'_>, subject: String) {
        self.violations.push(Violation {
            rule: RuleId::Migration(rule),
            line: call.line,
            column: call.column,
            subject,
            fix: None,
        });
    }

    fn call(&mut self, call: &Call<'_>, source: &[u8]) {
        if call.method == EXECUTE {
            self.execute(call, source);
            return;
        }
        let batch_table = self.batches.get(call.receiver).cloned();
        if call.receiver != OPERATIONS && batch_table.is_none() {
            return;
        }
        let offset = usize::from(batch_table.is_none());
        let table = batch_table.or_else(|| call.text_argument(0, "table_name"));
        let table = table.unwrap_or_default();
        match call.method {
            "create_table" => {
                self.created_tables
                    .insert(call.text_argument(0, "table_name").unwrap_or_default());
            }
            "create_index" => self.create_index(call, offset),
            "drop_index" if !call.keyword_is_true("postgresql_concurrently") => {
                let name = call.text_argument(0, "index_name").unwrap_or_default();
                self.report(MigrationRule::RequireConcurrentIndexDeletion, call, name);
            }
            "add_column" => self.add_column(call, offset, source),
            "alter_column" => self.alter_column(call, offset),
            "drop_column" => {
                let column = call.text_argument(offset, "column_name").unwrap_or_default();
                self.report(MigrationRule::BanDropColumn, call, column);
            }
            "drop_table" => self.report(MigrationRule::BanDropTable, call, table),
            "rename_table" => self.report(MigrationRule::RenamingTable, call, table),
            _ => {}
        }
    }

    fn create_index(&mut self, call: &Call<'_>, offset: usize) {
        if call.keyword_is_true("postgresql_concurrently") {
            return;
        }
        let table = self.table_of(call, offset, 1);
        self.unless_created(call, table);
    }

    fn add_column(&mut self, call: &Call<'_>, offset: usize, source: &[u8]) {
        let Some(column) = call.positional(offset).and_then(|node| Call::parse(node, source)) else {
            return;
        };
        let nullable = column.keyword("nullable").map(|value| value.kind());
        let primary = column.keyword_is_true("primary_key");
        let required = nullable == Some("false") || (primary && nullable != Some("true"));
        let generated = column
            .all_positional()
            .iter()
            .filter_map(|argument| Call::parse(*argument, source))
            .any(|argument| argument.method == "Computed" || argument.method == "Identity");
        if required && column.keyword("server_default").is_none() && !generated {
            let name = column.text_argument(0, "name").unwrap_or_default();
            self.report(MigrationRule::AddingRequiredField, call, name);
        }
        if column.keyword_is_true("index") {
            let table = self.table_of(call, offset, 0);
            self.unless_created(call, table);
        }
    }

    fn table_of(&self, call: &Call<'_>, offset: usize, position: usize) -> String {
        let table = if offset == 0 {
            self.batches.get(call.receiver).cloned()
        } else {
            call.text_argument(position, "table_name")
        };
        table.unwrap_or_default()
    }

    fn unless_created(&mut self, call: &Call<'_>, table: String) {
        if !self.created_tables.contains(&table) {
            self.report(MigrationRule::RequireConcurrentIndexCreation, call, table);
        }
    }

    fn alter_column(&mut self, call: &Call<'_>, offset: usize) {
        let column = call.text_argument(offset, "column_name").unwrap_or_default();
        if call
            .keyword("nullable")
            .is_some_and(|value| value.kind() == "false")
        {
            self.report(MigrationRule::AddingNotNullableField, call, column.clone());
        }
        if call.keyword("type_").is_some() {
            self.report(MigrationRule::ChangingColumnType, call, column.clone());
        }
        if call.keyword("new_column_name").is_some() {
            self.report(MigrationRule::RenamingColumn, call, column);
        }
    }

    fn execute(&mut self, call: &Call<'_>, source: &[u8]) {
        let Some(statement) = call.positional(0) else {
            return;
        };
        let literal = Call::parse(statement, source)
            .filter(|inner| inner.method == "text")
            .and_then(|inner| inner.positional(0))
            .unwrap_or(statement);
        if let Some(sql) = string_value(literal, source) {
            let first_line = one_based(literal.start_position().row);
            self.violations.extend(super::sql::violations(&sql, first_line));
        }
    }
}

#[cfg(test)]
mod tests;
