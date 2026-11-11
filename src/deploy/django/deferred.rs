use super::names::{index_name, quoted};

const EXPRESSION_START: char = '(';
const DESCENDING_PREFIX: char = '-';
const UNIQUE_SUFFIX: &str = "_uniq";
const LIKE_SUFFIX: &str = "_like";
const DESCENDING_ORDER: &str = "DESC";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    Plain,
    Unique,
    Concurrent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexColumn {
    pub name: String,
    pub ordering: Option<String>,
    pub operator_class: Option<String>,
}

impl IndexColumn {
    pub fn parse(entry: &str, column_of: impl Fn(&str) -> String) -> Self {
        entry.strip_prefix(DESCENDING_PREFIX).map_or_else(
            || Self {
                name: if entry.starts_with(EXPRESSION_START) {
                    entry.to_owned()
                } else {
                    column_of(entry)
                },
                ordering: None,
                operator_class: None,
            },
            |name| Self {
                name: column_of(name),
                ordering: Some(DESCENDING_ORDER.to_owned()),
                operator_class: None,
            },
        )
    }

    fn render(&self) -> String {
        let name = if self.name.starts_with(EXPRESSION_START) {
            self.name.clone()
        } else {
            quoted(&self.name)
        };
        let name = match &self.operator_class {
            Some(operator_class) => format!("{name} {operator_class}"),
            None => name,
        };
        match &self.ordering {
            Some(ordering) => format!("{name} {ordering}"),
            None => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Index,
    Like(&'static str),
    Unique,
    ForeignKey {
        to_table: String,
        to_column: String,
    },
    Named {
        name: String,
        kind: IndexKind,
        condition: Option<String>,
        nulls_distinct: Option<bool>,
        method: Option<&'static str>,
        storage: Vec<String>,
    },
    Constraint {
        name: String,
        definition: String,
    },
    Alter(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub table: String,
    pub columns: Vec<IndexColumn>,
    pub kind: Kind,
}

impl Statement {
    pub fn on_columns(table: &str, columns: &[String], kind: Kind) -> Self {
        Self {
            table: table.to_owned(),
            columns: columns
                .iter()
                .map(|name| IndexColumn {
                    name: name.clone(),
                    ordering: None,
                    operator_class: None,
                })
                .collect(),
            kind,
        }
    }

    fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|column| column.name.as_str()).collect()
    }

    fn column_list(&self) -> String {
        self.columns
            .iter()
            .map(IndexColumn::render)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn render(&self) -> String {
        let table = quoted(&self.table);
        let columns = self.column_list();
        let names = self.column_names();
        match &self.kind {
            Kind::Index => format!(
                "CREATE INDEX {} ON {table} ({columns})",
                quoted(&index_name(&self.table, &names, ""))
            ),
            Kind::Like(operator_class) => format!(
                "CREATE INDEX {} ON {table} ({columns} {operator_class})",
                quoted(&index_name(&self.table, &names, LIKE_SUFFIX))
            ),
            Kind::Unique => format!(
                "ALTER TABLE {table} ADD CONSTRAINT {} UNIQUE ({columns})",
                quoted(&index_name(&self.table, &names, UNIQUE_SUFFIX))
            ),
            Kind::ForeignKey { to_table, to_column } => format!(
                "ALTER TABLE {table} ADD CONSTRAINT {} FOREIGN KEY ({columns}) REFERENCES {} ({}) DEFERRABLE INITIALLY DEFERRED",
                quoted(&foreign_key_name(&self.table, &names, to_table, to_column)),
                quoted(to_table),
                quoted(to_column)
            ),
            Kind::Named {
                name,
                kind,
                condition,
                nulls_distinct,
                method,
                storage,
            } => {
                let create = match kind {
                    IndexKind::Plain => "CREATE INDEX",
                    IndexKind::Unique => "CREATE UNIQUE INDEX",
                    IndexKind::Concurrent => "CREATE INDEX CONCURRENTLY",
                };
                let nulls = nulls_clause(*nulls_distinct);
                let condition = condition
                    .as_ref()
                    .map(|sql| format!(" WHERE {sql}"))
                    .unwrap_or_default();
                let using = method
                    .map(|method| format!(" USING {method}"))
                    .unwrap_or_default();
                let storage = if storage.is_empty() {
                    String::new()
                } else {
                    format!(" WITH ({})", storage.join(", "))
                };
                format!(
                    "{create} {} ON {table}{using} ({columns}){nulls}{storage}{condition}",
                    quoted(name)
                )
            }
            Kind::Constraint { name, definition } => {
                format!("ALTER TABLE {table} ADD CONSTRAINT {} {definition}", quoted(name))
            }
            Kind::Alter(changes) => format!("ALTER TABLE {table} {changes}"),
        }
    }

    pub fn references_table(&self, table: &str) -> bool {
        self.table == table || matches!(&self.kind, Kind::ForeignKey { to_table, .. } if to_table == table)
    }

    pub fn references_column(&self, table: &str, column: &str) -> bool {
        let own = self.table == table && self.columns.iter().any(|entry| entry.name == column);
        own || matches!(&self.kind, Kind::ForeignKey { to_table, to_column } if to_table == table && to_column == column)
    }

    pub fn rename_table(&mut self, old: &str, new: &str) {
        if self.table == old {
            new.clone_into(&mut self.table);
        }
        if let Kind::ForeignKey { to_table, .. } = &mut self.kind
            && to_table == old
        {
            new.clone_into(to_table);
        }
    }

    pub fn rename_column(&mut self, table: &str, old: &str, new: &str) {
        if self.table == table {
            for column in self.columns.iter_mut().filter(|column| column.name == old) {
                new.clone_into(&mut column.name);
            }
        }
        if let Kind::ForeignKey { to_table, to_column } = &mut self.kind
            && to_table == table
            && to_column == old
        {
            new.clone_into(to_column);
        }
    }
}

pub fn foreign_key_name(table: &str, columns: &[&str], to_table: &str, to_column: &str) -> String {
    index_name(table, columns, &format!("_fk_{to_table}_{to_column}"))
}

pub fn like_index_name(table: &str, column: &str) -> String {
    index_name(table, &[column], LIKE_SUFFIX)
}

pub const fn nulls_clause(nulls_distinct: Option<bool>) -> &'static str {
    match nulls_distinct {
        Some(true) => " NULLS DISTINCT",
        Some(false) => " NULLS NOT DISTINCT",
        None => "",
    }
}
