use std::collections::BTreeMap;

use super::ddl::{index_name, quote};
use super::sql_tokens::{Cursor, Token, statements};

const PRIMARY_KEY_SUFFIX: &str = "_pkey";
const SEQUENCE_SUFFIX: &str = "_seq";
const SERIAL_TYPES: &[&str] = &["serial", "bigserial", "smallserial"];

#[derive(Debug, Clone, Default)]
struct Index {
    name: String,
    columns: Vec<String>,
}

#[derive(Debug, Clone)]
struct PrimaryKey {
    column: String,
    serial: bool,
}

#[derive(Debug, Clone)]
struct ForeignKey {
    name: String,
    column: String,
}

#[derive(Debug, Clone, Default)]
struct Table {
    primary_key: Option<PrimaryKey>,
    indexes: Vec<Index>,
    foreign_keys: Vec<ForeignKey>,
}

#[derive(Debug, Clone, Default)]
pub struct Schema {
    complete: bool,
    tables: BTreeMap<String, Table>,
}

enum Lookup<'schema> {
    Found(&'schema Table),
    Missing,
    Unknown,
}

fn rename_index(from: &str, to: &str) -> String {
    format!("ALTER INDEX {} RENAME TO {}", quote(from), quote(to))
}

impl Schema {
    pub const fn complete() -> Self {
        Self {
            complete: true,
            tables: BTreeMap::new(),
        }
    }

    fn lookup(&self, table: &str) -> Lookup<'_> {
        match self.tables.get(table) {
            Some(found) => Lookup::Found(found),
            None if self.complete => Lookup::Missing,
            None => Lookup::Unknown,
        }
    }

    fn check(&self, table: &str, predicate: impl Fn(&Table) -> bool) -> Option<bool> {
        match self.lookup(table) {
            Lookup::Found(found) => Some(predicate(found)),
            Lookup::Missing => Some(false),
            Lookup::Unknown => None,
        }
    }

    pub fn table_exists(&self, table: &str) -> Option<bool> {
        self.check(table, |_| true)
    }

    pub fn index_exists(&self, table: &str, columns: &[String], name: Option<&str>) -> Option<bool> {
        self.check(table, |found| {
            found.indexes.iter().any(|index| {
                name.is_none_or(|name| index.name == name) && (columns.is_empty() || index.columns == columns)
            })
        })
    }

    pub fn foreign_key_exists(&self, table: &str, column: &str, name: Option<&str>) -> Option<bool> {
        self.check(table, |found| {
            found
                .foreign_keys
                .iter()
                .any(|key| name.map_or(key.column == column, |name| key.name == name))
        })
    }

    pub fn drops_missing_object(&self, table: &str, statement: &str) -> bool {
        match self.lookup(table) {
            Lookup::Found(found) => statements(statement)
                .iter()
                .all(|tokens| found.lacks_dropped_object(tokens)),
            Lookup::Missing => true,
            Lookup::Unknown => false,
        }
    }

    pub fn apply(&mut self, sql: &str) -> Vec<String> {
        statements(sql)
            .iter()
            .flat_map(|tokens| self.apply_statement(tokens))
            .collect()
    }

    fn apply_statement(&mut self, tokens: &[Token]) -> Vec<String> {
        let mut cursor = Cursor::new(tokens);
        if cursor.keyword("create") {
            self.create(&mut cursor);
        } else if cursor.keywords(&["drop", "index"]) {
            cursor.keyword("concurrently");
            cursor.keywords(&["if", "exists"]);
            for name in cursor.identifiers() {
                self.tables
                    .values_mut()
                    .for_each(|table| table.indexes.retain(|index| index.name != name));
            }
        } else if cursor.keywords(&["drop", "table"]) {
            cursor.keywords(&["if", "exists"]);
            for name in cursor.identifiers() {
                self.tables.remove(&name);
            }
        } else if cursor.keywords(&["alter", "index"]) {
            cursor.keywords(&["if", "exists"]);
            if let (Some(from), true, Some(to)) = (
                cursor.identifier(),
                cursor.keywords(&["rename", "to"]),
                cursor.identifier(),
            ) {
                self.tables
                    .values_mut()
                    .flat_map(|table| table.indexes.iter_mut())
                    .filter(|index| index.name == from)
                    .for_each(|index| index.name.clone_from(&to));
            }
        } else if cursor.keywords(&["alter", "table"]) {
            cursor.keywords(&["if", "exists"]);
            cursor.keyword("only");
            if let Some(table) = cursor.identifier() {
                return self.alter_table(&table, &cursor);
            }
        }
        Vec::new()
    }

    fn create(&mut self, cursor: &mut Cursor<'_>) {
        cursor.keyword("unique");
        if cursor.keyword("index") {
            self.create_index(cursor);
            return;
        }
        cursor.skip_until_keyword("table");
        if cursor.keyword("table") {
            self.create_table(cursor);
        }
    }

    fn create_index(&mut self, cursor: &mut Cursor<'_>) {
        cursor.keyword("concurrently");
        cursor.keywords(&["if", "not", "exists"]);
        let Some(name) = cursor.identifier().filter(|name| name != "on") else {
            return;
        };
        cursor.keyword("on");
        cursor.keyword("only");
        let Some(table) = cursor.identifier() else {
            return;
        };
        let columns = cursor
            .elements()
            .into_iter()
            .map(|element| Cursor::single_identifier(element).unwrap_or_else(|| Cursor::render(element)))
            .collect();
        let indexes = &mut self.tables.entry(table).or_default().indexes;
        indexes.retain(|index| index.name != name);
        indexes.push(Index { name, columns });
    }

    fn create_table(&mut self, cursor: &mut Cursor<'_>) {
        cursor.keywords(&["if", "not", "exists"]);
        let Some(name) = cursor.identifier() else {
            return;
        };
        let mut table = Table::default();
        for element in cursor.elements() {
            let mut element = Cursor::new(element);
            if element.keyword("constraint") {
                let constraint = element.identifier().unwrap_or_default();
                table.add_constraint(constraint, &mut element);
            } else if element.contains_sequence(&["primary", "key"]) {
                let serial = SERIAL_TYPES.iter().any(|kind| element.contains(kind));
                table.primary_key = element.identifier().map(|column| PrimaryKey { column, serial });
            }
        }
        self.tables.insert(name, table);
    }

    fn alter_table(&mut self, table: &str, cursor: &Cursor<'_>) -> Vec<String> {
        let mut follow_ups = Vec::new();
        if !self.tables.contains_key(table) && self.complete {
            self.complete = false;
        }
        for action in cursor.actions() {
            let mut action = Cursor::new(action);
            if action.keywords(&["rename", "to"]) {
                let renamed = action.identifier().map(|to| self.rename_table(table, &to));
                follow_ups.extend(renamed.unwrap_or_default());
                return follow_ups;
            }
            if let Some(found) = self.tables.get_mut(table) {
                follow_ups.extend(found.alter(table, &mut action));
            }
        }
        follow_ups
    }

    fn rename_table(&mut self, from: &str, to: &str) -> Vec<String> {
        let Some(mut table) = self.tables.remove(from) else {
            return vec![rename_index(
                &format!("{from}{PRIMARY_KEY_SUFFIX}"),
                &format!("{to}{PRIMARY_KEY_SUFFIX}"),
            )];
        };
        let mut follow_ups = Vec::new();
        if let Some(primary_key) = &table.primary_key {
            follow_ups.push(rename_index(
                &format!("{from}{PRIMARY_KEY_SUFFIX}"),
                &format!("{to}{PRIMARY_KEY_SUFFIX}"),
            ));
            if primary_key.serial {
                let column = &primary_key.column;
                follow_ups.push(format!(
                    "ALTER TABLE {} RENAME TO {}",
                    quote(&format!("{from}_{column}{SEQUENCE_SUFFIX}")),
                    quote(&format!("{to}_{column}{SEQUENCE_SUFFIX}"))
                ));
            }
        }
        table.indexes.sort_by(|left, right| left.name.cmp(&right.name));
        for index in &mut table.indexes {
            if index.name == index_name(from, &index.columns) {
                let renamed = index_name(to, &index.columns);
                follow_ups.push(rename_index(&index.name, &renamed));
                index.name = renamed;
            }
        }
        self.tables.insert(to.to_owned(), table);
        follow_ups
    }
}

impl Table {
    fn lacks_dropped_object(&self, tokens: &[Token]) -> bool {
        let mut cursor = Cursor::new(tokens);
        if cursor.keywords(&["drop", "index"]) {
            cursor.keyword("concurrently");
            cursor.keywords(&["if", "exists"]);
            let name = cursor.identifier().unwrap_or_default();
            return self.indexes.iter().all(|index| index.name != name);
        }
        let name = cursor
            .actions()
            .first()
            .and_then(|action| {
                let mut action = Cursor::new(action);
                action.skip_until_keyword("constraint");
                action.keyword("constraint").then(|| action.identifier())?
            })
            .unwrap_or_default();
        self.foreign_keys.iter().all(|key| key.name != name)
    }

    fn alter(&mut self, table: &str, action: &mut Cursor<'_>) -> Vec<String> {
        if action.keyword("rename") {
            action.keyword("column");
            let from = action.identifier();
            let to = action.keyword("to").then(|| action.identifier()).flatten();
            if let (Some(from), Some(to)) = (from, to) {
                return self.rename_column(table, &from, &to);
            }
        } else if action.keyword("drop") {
            self.drop_object(action);
        } else if action.keywords(&["add", "constraint"]) {
            let constraint = action.identifier().unwrap_or_default();
            self.add_constraint(constraint, action);
        }
        Vec::new()
    }

    fn drop_object(&mut self, action: &mut Cursor<'_>) {
        if action.keyword("constraint") {
            action.keywords(&["if", "exists"]);
            let name = action.identifier().unwrap_or_default();
            self.foreign_keys.retain(|key| key.name != name);
            return;
        }
        action.keyword("column");
        action.keywords(&["if", "exists"]);
        if let Some(column) = action.identifier() {
            self.drop_column(&column);
        }
    }

    fn add_constraint(&mut self, name: String, cursor: &mut Cursor<'_>) {
        if cursor.keywords(&["foreign", "key"]) {
            if let Some(column) = cursor
                .elements()
                .first()
                .and_then(|column| Cursor::single_identifier(column))
            {
                self.foreign_keys.push(ForeignKey { name, column });
            }
        } else if cursor.keyword("unique") {
            let columns = cursor
                .elements()
                .into_iter()
                .filter_map(Cursor::single_identifier)
                .collect();
            self.indexes.push(Index { name, columns });
        }
    }

    fn drop_column(&mut self, column: &str) {
        self.indexes
            .retain(|index| !index.columns.iter().any(|indexed| indexed == column));
        self.foreign_keys.retain(|key| key.column != column);
    }

    fn rename_column(&mut self, table: &str, from: &str, to: &str) -> Vec<String> {
        for index in &mut self.indexes {
            index
                .columns
                .iter_mut()
                .filter(|column| *column == from)
                .for_each(|column| to.clone_into(column));
        }
        self.foreign_keys
            .iter_mut()
            .filter(|key| key.column == from)
            .for_each(|key| to.clone_into(&mut key.column));
        self.indexes.sort_by(|left, right| left.name.cmp(&right.name));
        let mut follow_ups = Vec::new();
        for index in &mut self.indexes {
            let Some(position) = index.columns.iter().position(|column| column == to) else {
                continue;
            };
            let mut previous = index.columns.clone();
            if let Some(column) = previous.get_mut(position) {
                from.clone_into(column);
            }
            if index.name == index_name(table, &previous) {
                let renamed = index_name(table, &index.columns);
                follow_ups.push(rename_index(&index.name, &renamed));
                index.name = renamed;
            }
        }
        follow_ups
    }
}
