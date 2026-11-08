use std::mem;

use super::columns::{check, definition, effective_default, has_like_index, identity, like_operator_class};
use super::deferred::{IndexColumn, IndexKind, Kind, Statement, foreign_key_name};
use super::model::{Constraint, ConstraintKind, Field, Index, Model};
use super::names::quoted;
use super::state::State;
use crate::deploy::Violation;
use crate::deploy::script::{Origin, Script};

const EXPRESSION_PLACEHOLDER: &str = "(true)";
const STATEMENT_TERMINATOR: char = ';';

#[derive(Debug)]
pub struct Editor {
    script: Script,
    deferred: Vec<(Statement, Origin)>,
    origin: Origin,
    atomic: bool,
}

impl Editor {
    pub fn new(atomic: bool) -> Self {
        Self {
            script: if atomic {
                Script::new()
            } else {
                Script::without_transaction()
            },
            deferred: Vec::new(),
            origin: Origin::Synthetic,
            atomic,
        }
    }

    pub const fn at(&mut self, origin: Origin) {
        self.origin = origin;
    }

    pub fn execute(&mut self, sql: &str) {
        self.script.push(sql, self.origin);
    }

    pub fn execute_raw(&mut self, sql: &str, first_line: usize) {
        let trimmed = sql.trim_end().trim_end_matches(STATEMENT_TERMINATOR);
        if trimmed.trim().is_empty() {
            return;
        }
        self.script.push(
            trimmed,
            Origin::Execute {
                first_line,
                script_line: 0,
            },
        );
    }

    pub fn run(&mut self, statement: &Statement) {
        self.execute(&statement.render());
    }

    pub fn defer(&mut self, statement: Statement) {
        self.deferred.push((statement, self.origin));
    }

    pub fn rename_deferred_column(&mut self, table: &str, old: &str, new: &str) {
        for (statement, _) in &mut self.deferred {
            statement.rename_column(table, old, new);
        }
    }

    pub fn finish(mut self) -> Vec<Violation> {
        for (statement, origin) in mem::take(&mut self.deferred) {
            self.script.push(&statement.render(), origin);
        }
        if self.atomic {
            self.script.leave_transaction();
        }
        self.script.finish()
    }

    pub fn create_model(&mut self, model: &Model, state: &State) {
        if !model.emits_sql() {
            return;
        }
        let table = model.table();
        for set in &model.unique_together {
            self.defer(Statement::on_columns(
                &table,
                &columns_of(model, set),
                Kind::Unique,
            ));
        }
        let mut definitions: Vec<String> = model
            .local_fields()
            .map(|field| self.column_definition(model, field, state))
            .collect();
        definitions.extend(
            model
                .constraints
                .iter()
                .filter_map(|constraint| self.inline_constraint(model, constraint)),
        );
        self.execute(&format!(
            "CREATE TABLE {} ({})",
            quoted(&table),
            definitions.join(", ")
        ));
        for field in model.local_fields() {
            self.defer_field_indexes(&table, field, state);
        }
        for index in &model.indexes {
            self.defer(named_index(model, index, IndexKind::Plain));
        }
        self.create_throughs(model, state);
    }

    fn create_throughs(&mut self, model: &Model, state: &State) {
        for field in model.fields.iter().filter(|field| field.auto_through()) {
            if let Some(through) = state.through(model, field) {
                self.create_model(&through, state);
            }
        }
    }

    fn column_definition(&mut self, model: &Model, field: &Field, state: &State) -> String {
        let mut parts = vec![quoted(&field.column()), definition(field, state, false)];
        parts.extend(check(field).map(|check| format!("CHECK ({check})")));
        parts.extend(identity(field).map(str::to_owned));
        if let Some((to_table, to_column)) = constrained_target(field, state) {
            self.defer(Statement::on_columns(
                &model.table(),
                &[field.column()],
                Kind::ForeignKey { to_table, to_column },
            ));
        }
        parts.join(" ")
    }

    fn inline_constraint(&mut self, model: &Model, constraint: &Constraint) -> Option<String> {
        let name = quoted(&constraint.name);
        match &constraint.kind {
            ConstraintKind::Unique { by_index: true, .. } => {
                self.defer(unique_index(model, constraint));
                None
            }
            ConstraintKind::Unique { fields, .. } => Some(format!(
                "CONSTRAINT {name} UNIQUE ({})",
                quoted_columns(&columns_of(model, fields))
            )),
            ConstraintKind::Check => Some(format!("CONSTRAINT {name} CHECK (true)")),
        }
    }

    pub fn defer_field_indexes(&mut self, table: &str, field: &Field, state: &State) {
        let columns = [field.column()];
        if field.db_index() && !field.unique() {
            self.defer(Statement::on_columns(table, &columns, Kind::Index));
        }
        if has_like_index(field, state) {
            self.defer(Statement::on_columns(
                table,
                &columns,
                Kind::Like(like_operator_class(field, state)),
            ));
        }
    }

    pub fn delete_model(&mut self, model: &Model, state: &State) {
        if !model.emits_sql() {
            return;
        }
        for field in model.fields.iter().filter(|field| field.auto_through()) {
            if let Some(through) = state.through(model, field) {
                self.delete_model(&through, state);
            }
        }
        let table = model.table();
        self.execute(&format!("DROP TABLE {} CASCADE", quoted(&table)));
        self.deferred
            .retain(|(statement, _)| !statement.references_table(&table));
    }

    pub fn add_field(&mut self, model: &Model, field: &Field, state: &State) {
        if field.auto_through() {
            if let Some(through) = state.through(model, field) {
                self.create_model(&through, state);
            }
            return;
        }
        if field.is_many_to_many() {
            return;
        }
        let table = model.table();
        let column = quoted(&field.column());
        let mut parts = vec![definition(field, state, true)];
        parts.extend(identity(field).map(str::to_owned));
        parts.extend(check(field).map(|check| format!("CHECK ({check})")));
        if let Some((to_table, to_column)) = constrained_target(field, state) {
            let name = quoted(&foreign_key_name(
                &table,
                &[&field.column()],
                &to_table,
                &to_column,
            ));
            parts.push(format!(
                "CONSTRAINT {name} REFERENCES {}({}) DEFERRABLE INITIALLY DEFERRED; SET CONSTRAINTS {name} IMMEDIATE",
                quoted(&to_table),
                quoted(&to_column)
            ));
        }
        let column_definition = parts.join(" ");
        let quoted_table = quoted(&table);
        self.execute(&format!(
            "ALTER TABLE {quoted_table} ADD COLUMN {column} {column_definition}"
        ));
        if field.db_default.is_none() && effective_default(field).is_some() {
            self.execute(&format!(
                "ALTER TABLE {quoted_table} ALTER COLUMN {column} DROP DEFAULT"
            ));
        }
        self.defer_field_indexes(&table, field, state);
    }

    pub fn remove_field(&mut self, model: &Model, field: &Field, state: &State) {
        if field.auto_through() {
            if let Some(through) = state.through(model, field) {
                self.delete_model(&through, state);
            }
            return;
        }
        if field.is_many_to_many() {
            return;
        }
        let table = model.table();
        let column = field.column();
        self.execute(&format!(
            "ALTER TABLE {} DROP COLUMN {}",
            quoted(&table),
            quoted(&column)
        ));
        self.deferred
            .retain(|(statement, _)| !statement.references_column(&table, &column));
    }

    pub fn rename_table(&mut self, old: &str, new: &str) {
        if old == new {
            return;
        }
        self.execute(&format!("ALTER TABLE {} RENAME TO {}", quoted(old), quoted(new)));
        for (statement, _) in &mut self.deferred {
            statement.rename_table(old, new);
        }
    }
}

pub fn constrained_target(field: &Field, state: &State) -> Option<(String, String)> {
    let relation = field.relation.as_ref()?;
    if !field.is_foreign_key() || !field.db_constraint() {
        return None;
    }
    let target = state.resolve(&relation.to);
    let column = relation
        .to_field
        .as_deref()
        .map(|name| target.column_of(name))
        .or_else(|| target.primary_key().map(Field::column))?;
    Some((target.table(), column))
}

pub fn columns_of(model: &Model, names: &[String]) -> Vec<String> {
    names.iter().map(|name| model.column_of(name)).collect()
}

pub fn quoted_columns(columns: &[String]) -> String {
    columns
        .iter()
        .map(|column| quoted(column))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn named_index(model: &Model, index: &Index, kind: IndexKind) -> Statement {
    Statement {
        table: model.table(),
        columns: index
            .fields
            .iter()
            .map(|entry| IndexColumn::parse(entry, |name| model.column_of(name)))
            .collect(),
        kind: Kind::Named {
            name: index.name.clone(),
            kind,
            conditional: index.conditional,
        },
    }
}

pub fn unique_index(model: &Model, constraint: &Constraint) -> Statement {
    let fields = match &constraint.kind {
        ConstraintKind::Unique { fields, .. } if !fields.is_empty() => fields.clone(),
        ConstraintKind::Unique { .. } | ConstraintKind::Check => vec![EXPRESSION_PLACEHOLDER.to_owned()],
    };
    named_index(
        model,
        &Index {
            name: constraint.name.clone(),
            fields,
            conditional: false,
        },
        IndexKind::Unique,
    )
}
