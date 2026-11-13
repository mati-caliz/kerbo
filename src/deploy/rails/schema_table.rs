use super::ddl::{index_name, quote};
use super::sql_tokens::{Cursor, Token};

pub const TABLE_CONSTRAINTS: &[&str] = &[
    "constraint",
    "primary",
    "unique",
    "check",
    "foreign",
    "exclude",
    "like",
];

pub fn rename_index(from: &str, to: &str) -> String {
    format!("ALTER INDEX {} RENAME TO {}", quote(from), quote(to))
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PrimaryKey {
    pub column: String,
    pub serial: bool,
    pub defaulted: bool,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub name: String,
    pub column: String,
}

#[derive(Debug, Clone, Default)]
pub struct Table {
    pub columns: Option<Vec<String>>,
    pub primary_key: Option<PrimaryKey>,
    pub indexes: Vec<Index>,
    pub foreign_keys: Vec<ForeignKey>,
}

impl Table {
    pub fn lacks_dropped_object(&self, tokens: &[Token]) -> bool {
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

    pub fn alter(&mut self, table: &str, action: &mut Cursor<'_>) -> Vec<String> {
        if action.keyword("rename") && !action.at_keyword("constraint") {
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
        } else if action.keyword("add") {
            self.add_column(action);
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

    pub fn add_constraint(&mut self, name: String, cursor: &mut Cursor<'_>) {
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

    fn add_column(&mut self, action: &mut Cursor<'_>) {
        action.keyword("column");
        action.keywords(&["if", "not", "exists"]);
        if action.at_any_keyword(TABLE_CONSTRAINTS) {
            return;
        }
        if let (Some(columns), Some(column)) = (self.columns.as_mut(), action.identifier())
            && !columns.contains(&column)
        {
            columns.push(column);
        }
    }

    fn drop_column(&mut self, column: &str) {
        if let Some(columns) = self.columns.as_mut() {
            columns.retain(|known| known != column);
        }
        self.indexes
            .retain(|index| !index.columns.iter().any(|indexed| indexed == column));
        self.foreign_keys.retain(|key| key.column != column);
    }

    fn rename_column(&mut self, table: &str, from: &str, to: &str) -> Vec<String> {
        self.columns
            .iter_mut()
            .flatten()
            .filter(|column| *column == from)
            .for_each(|column| to.clone_into(column));
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
