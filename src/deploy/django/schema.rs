use std::mem;

use super::columns::{
    bare_definition, check, column_comment, definition, effective_default, has_like_index, identity,
    like_operator_class,
};
use super::deferred::{IndexKind, Kind, Statement, foreign_key_name};
use super::field_defaults::NOW_SQL;
use super::model::{Constraint, ConstraintKind, Field, Model};
use super::names::{index_name, quoted, sql_string};
use super::state::State;
use super::table_constraints::named_index;
use super::table_constraints::{constraint_definition, unique_index};
use super::zero_downtime::{BTREE_GIST_EXTENSION, Backend, translate};
use crate::deploy::Violation;
use crate::deploy::script::{Origin, Script};

const STATEMENT_TERMINATOR: char = ';';
const CASCADE_SUFFIX: &str = " CASCADE";
const SQL_NULL: &str = "NULL";
const PRIMARY_KEY_SUFFIX: &str = "_pk";

#[derive(Debug)]
pub struct Editor {
    script: Script,
    deferred: Vec<(Statement, Origin)>,
    origin: Origin,
    atomic: bool,
    zero_downtime: Option<Backend>,
}

impl Editor {
    pub fn new(atomic: bool, zero_downtime: Option<Backend>) -> Self {
        Self {
            script: if atomic {
                Script::new()
            } else {
                Script::without_transaction()
            },
            deferred: Vec::new(),
            origin: Origin::Synthetic,
            atomic,
            zero_downtime,
        }
    }

    pub const fn zero_downtime(&self) -> Option<Backend> {
        self.zero_downtime
    }

    pub const fn at(&mut self, origin: Origin) {
        self.origin = origin;
    }

    pub fn execute(&mut self, sql: &str) {
        let origin = self.origin;
        self.push_translated(sql, origin);
    }

    fn push_translated(&mut self, sql: &str, origin: Origin) {
        if self.zero_downtime.is_none() {
            self.script.push(sql, origin);
            return;
        }
        for statement in translate(sql) {
            self.script.push(&statement, origin);
        }
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

    pub fn rename_column(&mut self, table: &str, old: &str, new: &str) {
        self.execute(&format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            quoted(table),
            quoted(old),
            quoted(new)
        ));
        for (statement, _) in &mut self.deferred {
            statement.rename_column(table, old, new);
        }
    }

    pub fn finish(self) -> Vec<Violation> {
        self.into_script().finish()
    }

    pub fn into_script(mut self) -> Script {
        for (statement, origin) in mem::take(&mut self.deferred) {
            self.push_translated(&statement.render(), origin);
        }
        if self.atomic {
            self.script.leave_transaction();
        }
        self.script
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
        let has_exclusion = model
            .constraints
            .iter()
            .any(|constraint| matches!(constraint.kind, ConstraintKind::Exclusion { .. }));
        if has_exclusion
            && self
                .zero_downtime
                .is_some_and(|backend| backend.editor.ensures_btree_gist)
        {
            self.execute(BTREE_GIST_EXTENSION);
        }
        let mut definitions: Vec<String> = model
            .local_fields()
            .map(|field| self.column_definition(model, field, state))
            .collect();
        let deferred_constraints = model.local_fields().any(has_parametrized_default);
        for constraint in &model.constraints {
            definitions.extend(self.table_constraint(model, constraint, deferred_constraints));
        }
        self.execute(&format!(
            "CREATE TABLE {} ({})",
            quoted(&table),
            definitions.join(", ")
        ));
        if let Some(comment) = &model.table_comment {
            self.execute(&format!(
                "COMMENT ON TABLE {} IS {}",
                quoted(&table),
                sql_string(comment)
            ));
        }
        for field in model.local_fields().filter(|field| field.db_comment.is_some()) {
            self.execute(&column_comment(&table, field));
        }
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

    fn table_constraint(&mut self, model: &Model, constraint: &Constraint, deferred: bool) -> Option<String> {
        let Some(definition) = constraint_definition(model, constraint) else {
            self.defer(unique_index(model, constraint));
            return None;
        };
        if deferred {
            self.defer(Statement::on_columns(
                &model.table(),
                &[],
                Kind::Constraint {
                    name: constraint.name.clone(),
                    definition,
                },
            ));
            return None;
        }
        Some(format!("CONSTRAINT {} {definition}", quoted(&constraint.name)))
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
        let zero_downtime = self.zero_downtime.is_some();
        let mut parts = vec![if zero_downtime {
            bare_definition(field, state, true)
        } else {
            definition(field, state, true)
        }];
        parts.extend(identity(field).map(str::to_owned));
        parts.extend(check(field).map(|check| format!("CHECK ({check})")));
        if zero_downtime {
            self.defer_added_constraints(&table, field, state);
        } else if let Some((to_table, to_column)) = constrained_target(field, state) {
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
        if field.db_comment.is_some() {
            self.execute(&column_comment(&table, field));
        }
        self.defer_field_indexes(&table, field, state);
    }

    fn defer_added_constraints(&mut self, table: &str, field: &Field, state: &State) {
        let columns = [field.column()];
        if field.primary_key() {
            self.defer(Statement::on_columns(
                table,
                &[],
                Kind::Constraint {
                    name: index_name(table, &[&field.column()], PRIMARY_KEY_SUFFIX),
                    definition: format!("PRIMARY KEY ({})", quoted(&field.column())),
                },
            ));
        } else if field.unique() {
            self.defer(Statement::on_columns(table, &columns, Kind::Unique));
        }
        if let Some((to_table, to_column)) = constrained_target(field, state) {
            self.defer(Statement::on_columns(
                table,
                &columns,
                Kind::ForeignKey { to_table, to_column },
            ));
        }
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
        let cascade = if state.column_drops_cascade() {
            CASCADE_SUFFIX
        } else {
            ""
        };
        self.execute(&format!(
            "ALTER TABLE {} DROP COLUMN {}{cascade}",
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

fn has_parametrized_default(field: &Field) -> bool {
    field
        .db_default
        .as_deref()
        .is_some_and(|database_default| database_default != SQL_NULL && database_default != NOW_SQL)
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
