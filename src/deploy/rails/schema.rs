use std::collections::BTreeMap;

use super::ddl::{index_name, quote};
use super::schema_table::{Index, IndexQuery, PrimaryKey, TABLE_CONSTRAINTS, Table, rename_index};
use super::sql_tokens::{Cursor, Token, statements};

const PRIMARY_KEY_SUFFIX: &str = "_pkey";
const SEQUENCE_SUFFIX: &str = "_seq";
const SERIAL_TYPES: &[&str] = &["serial", "bigserial", "smallserial"];
const KEY_DEFAULTS: &[&str] = &[
    "nextval",
    "gen_random_uuid",
    "uuid_generate_v4",
    "uuid_generate_v1",
    "identity",
];
const COLUMN_LIST_OPEN: char = '(';

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

    pub fn column_exists(&self, table: &str, column: &str) -> Option<bool> {
        match self.lookup(table) {
            Lookup::Found(found) => found
                .columns
                .as_ref()
                .map(|columns| columns.iter().any(|known| known == column)),
            Lookup::Missing => Some(false),
            Lookup::Unknown => None,
        }
    }

    pub fn index_exists(&self, table: &str, query: &IndexQuery<'_>) -> Option<bool> {
        self.check(table, |found| {
            found.indexes.iter().any(|index| query.matches(index))
        })
    }

    pub fn index_name_on(&self, table: &str, columns: &[String]) -> Option<String> {
        let Lookup::Found(found) = self.lookup(table) else {
            return None;
        };
        found
            .indexes
            .iter()
            .find(|index| index.columns == columns)
            .map(|index| index.name.clone())
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
        let unique = cursor.keyword("unique");
        if cursor.keyword("index") {
            self.create_index(cursor, unique);
            return;
        }
        cursor.skip_until_keyword("table");
        if cursor.keyword("table") {
            self.create_table(cursor);
        }
    }

    fn create_index(&mut self, cursor: &mut Cursor<'_>, unique: bool) {
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
        indexes.push(Index {
            name,
            columns,
            unique,
        });
    }

    fn create_table(&mut self, cursor: &mut Cursor<'_>) {
        cursor.keywords(&["if", "not", "exists"]);
        let Some(name) = cursor.identifier() else {
            return;
        };
        let mut table = Table::default();
        let mut columns = Vec::new();
        let lists_columns = cursor.at_symbol(COLUMN_LIST_OPEN);
        for tokens in cursor.elements() {
            let mut element = Cursor::new(tokens);
            if !element.at_any_keyword(TABLE_CONSTRAINTS)
                && let Some(column) = Cursor::new(tokens).identifier()
            {
                columns.push(column);
            }
            if element.keyword("constraint") {
                let constraint = element.identifier().unwrap_or_default();
                table.add_constraint(constraint, &mut element);
            } else if element.contains_sequence(&["primary", "key"]) {
                let serial = SERIAL_TYPES.iter().any(|kind| element.contains(kind));
                let defaulted = serial || KEY_DEFAULTS.iter().any(|default| element.contains(default));
                table.primary_key = element.identifier().map(|column| PrimaryKey {
                    column,
                    serial,
                    defaulted,
                });
            }
        }
        table.columns = lists_columns.then_some(columns);
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
        if let Some(primary_key) = table
            .primary_key
            .as_ref()
            .filter(|primary_key| primary_key.defaulted)
        {
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
