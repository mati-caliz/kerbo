use super::alter::{FieldChange, alter_field as alter_column};
use super::columns::db_type;
use super::deferred::{IndexKind, Kind, Statement, unique_together_name};
use super::many_to_many::{Side, alter_many_to_many};
use super::model::{Constraint, ConstraintKind, Model, ModelKey};
use super::names::{index_name, quoted};
use super::operations::{Deletion, Located, Migration, Operation};
use super::related;
use super::rename;
use super::schema::{Editor, columns_of};
use super::state::State;
use super::table_constraints::{constraint_definition, named_index, unique_index};
use super::zero_downtime::BTREE_GIST_EXTENSION;
use crate::deploy::Violation;
use crate::deploy::script::Origin;

pub const TABLE_PLACEHOLDER: &str = "{table}";
const NOT_VALID: &str = " NOT VALID";
const INDEX_TOGETHER_SUFFIX: &str = "_idx";

#[derive(Debug, Clone, Copy)]
struct Context<'context> {
    app: &'context str,
    before: &'context State,
    after: &'context State,
}

impl Context<'_> {
    fn key(&self, model: &str) -> ModelKey {
        ModelKey::new(self.app, model)
    }

    fn old_model(&self, model: &str) -> Option<&Model> {
        self.before
            .model(&self.key(model))
            .filter(|found| found.emits_sql())
    }

    fn new_model(&self, model: &str) -> Option<&Model> {
        self.after
            .model(&self.key(model))
            .filter(|found| found.emits_sql())
    }
}

pub fn script(migration: &Migration, app: &str, state: &mut State) -> Vec<Violation> {
    editor_after(migration, app, state).finish()
}

#[cfg(test)]
pub fn sql(migration: &Migration, app: &str, state: &mut State) -> String {
    editor_after(migration, app, state)
        .into_script()
        .text()
        .to_owned()
}

fn editor_after(migration: &Migration, app: &str, state: &mut State) -> Editor {
    let mut editor = Editor::new(migration.atomic, migration.zero_downtime);
    run_all(&mut editor, &migration.operations, app, state);
    editor
}

fn run_all(editor: &mut Editor, operations: &[Located], app: &str, state: &mut State) {
    for located in operations {
        if let Operation::Separate {
            database,
            state: state_operations,
        } = &located.operation
        {
            run_all(editor, database, app, &mut state.clone());
            for state_operation in state_operations {
                state.apply(app, &state_operation.operation);
            }
            continue;
        }
        let before = state.clone();
        state.apply(app, &located.operation);
        editor.at(Origin::Call {
            line: located.line,
            column: located.column,
        });
        let context = Context {
            app,
            before: &before,
            after: state,
        };
        render(editor, context, &located.operation);
    }
}

fn render(editor: &mut Editor, context: Context<'_>, operation: &Operation) {
    match operation {
        Operation::CreateModel(model) => {
            if let Some(created) = context.new_model(&model.key.model) {
                editor.create_model(created, context.after);
            }
        }
        Operation::DeleteModel { name, deletion } => {
            let deleted = match deletion {
                Deletion::Immediate => context.old_model(name),
                Deletion::MoveToPending => None,
                Deletion::Purge => context
                    .before
                    .pending_model(&context.key(name))
                    .filter(|found| found.emits_sql()),
            };
            if let Some(deleted) = deleted {
                editor.delete_model(deleted, context.before);
            }
        }
        Operation::RunSql(chunks) => {
            for chunk in chunks {
                editor.execute_raw(&chunk.sql, chunk.line);
            }
        }
        Operation::CreateExtension(name) => {
            editor.execute(&format!("CREATE EXTENSION IF NOT EXISTS {}", quoted(name)));
        }
        Operation::Statement { model, sql } => match model {
            None => editor.execute(sql),
            Some(model) => {
                if let Some(owner) = context.new_model(model) {
                    editor.execute(&sql.replace(TABLE_PLACEHOLDER, &quoted(&owner.table())));
                }
            }
        },
        Operation::RenameModel { old, new } => rename_model(editor, context, old, new),
        Operation::AlterModelTable { model, .. } => rename_model(editor, context, model, model),
        Operation::AddField { .. }
        | Operation::RemoveField { .. }
        | Operation::AlterField { .. }
        | Operation::RenameField { .. } => render_field(editor, context, operation),
        Operation::AlterUniqueTogether { .. }
        | Operation::AlterManaged { .. }
        | Operation::AddIndex { .. }
        | Operation::RemoveIndex { .. }
        | Operation::RenameIndex { .. }
        | Operation::AddConstraint { .. }
        | Operation::RemoveConstraint { .. }
        | Operation::Separate { .. } => render_meta(editor, context, operation),
    }
}

fn rename_model(editor: &mut Editor, context: Context<'_>, old: &str, new: &str) {
    if let (Some(old_model), Some(new_model)) = (context.old_model(old), context.new_model(new)) {
        rename::rename_model(editor, context.before, context.after, (old_model, new_model));
    }
}

fn render_field(editor: &mut Editor, context: Context<'_>, operation: &Operation) {
    match operation {
        Operation::AddField { model, field } => {
            if let Some(owner) = context.new_model(model) {
                editor.add_field(owner, field, context.after);
            }
        }
        Operation::RemoveField {
            model,
            name,
            deletion,
        } => {
            let owner = match deletion {
                Deletion::Immediate => context.old_model(model),
                Deletion::MoveToPending => None,
                Deletion::Purge => context
                    .before
                    .pending_field_owner(&context.key(model), name)
                    .filter(|found| found.emits_sql()),
            };
            if let Some(owner) = owner
                && let Some(field) = owner.field(name)
            {
                editor.remove_field(owner, field, context.before);
            }
        }
        Operation::AlterField { model, field } => {
            alter_field(editor, context, model, (&field.name, &field.name));
        }
        Operation::RenameField { model, old, new } => alter_field(editor, context, model, (old, new)),
        Operation::CreateModel(_)
        | Operation::DeleteModel { .. }
        | Operation::RenameModel { .. }
        | Operation::AlterModelTable { .. }
        | Operation::AlterUniqueTogether { .. }
        | Operation::AlterManaged { .. }
        | Operation::AddIndex { .. }
        | Operation::RemoveIndex { .. }
        | Operation::RenameIndex { .. }
        | Operation::AddConstraint { .. }
        | Operation::RemoveConstraint { .. }
        | Operation::RunSql(_)
        | Operation::CreateExtension(_)
        | Operation::Statement { .. }
        | Operation::Separate { .. } => {}
    }
}

fn alter_field(editor: &mut Editor, context: Context<'_>, model: &str, (old_name, new_name): (&str, &str)) {
    let (Some(old_model), Some(new_model)) = (context.old_model(model), context.new_model(model)) else {
        return;
    };
    let (Some(old), Some(new)) = (old_model.field(old_name), new_model.field(new_name)) else {
        return;
    };
    if old.is_many_to_many() && new.is_many_to_many() {
        let old_side = Side {
            owner: old_model,
            field: old,
            state: context.before,
        };
        let new_side = Side {
            owner: new_model,
            field: new,
            state: context.after,
        };
        alter_many_to_many(editor, &old_side, &new_side);
        return;
    }
    let table = new_model.table();
    let retypes_key =
        old.primary_key() && new.primary_key() && db_type(old, context.before) != db_type(new, context.after);
    let incoming = if retypes_key {
        related::incoming(&new_model.key, context.before, context.after)
    } else {
        Vec::new()
    };
    related::drop_constraints(editor, &incoming, (&old_model.table(), &old.column()));
    alter_column(
        editor,
        FieldChange {
            table: &table,
            old,
            new,
            before: context.before,
            after: context.after,
        },
    );
    related::alter_types(editor, &incoming, context.before, context.after);
    related::rebuild_constraints(editor, &incoming, (&table, &new.column()));
}

fn render_meta(editor: &mut Editor, context: Context<'_>, operation: &Operation) {
    match operation {
        Operation::AlterUniqueTogether { model, sets } => alter_unique_together(editor, context, model, sets),
        Operation::AddIndex {
            model,
            index,
            concurrently,
        } => {
            if let Some(owner) = context.new_model(model) {
                let kind = if *concurrently {
                    IndexKind::Concurrent
                } else {
                    IndexKind::Plain
                };
                editor.run(&named_index(owner, index, kind));
            }
        }
        Operation::RemoveIndex {
            model,
            name,
            concurrently,
        } => {
            if context.old_model(model).is_some() {
                let concurrent = if *concurrently { "CONCURRENTLY " } else { "" };
                editor.execute(&format!("DROP INDEX {concurrent}IF EXISTS {}", quoted(name)));
            }
        }
        Operation::RenameIndex {
            model,
            old,
            old_fields,
            new,
        } => {
            let old = old.clone().or_else(|| {
                context
                    .old_model(model)
                    .map(|owner| unnamed_index_name(owner, old_fields))
            });
            if context.new_model(model).is_some()
                && let Some(old) = old
            {
                editor.execute(&format!("ALTER INDEX {} RENAME TO {}", quoted(&old), quoted(new)));
            }
        }
        Operation::AddConstraint {
            model,
            constraint,
            not_valid,
        } => {
            if let Some(owner) = context.new_model(model) {
                let exclusion = matches!(constraint.kind, ConstraintKind::Exclusion { .. });
                if exclusion
                    && editor
                        .zero_downtime()
                        .is_some_and(|backend| backend.editor.ensures_btree_gist)
                {
                    editor.execute(BTREE_GIST_EXTENSION);
                }
                add_constraint(editor, owner, constraint, *not_valid);
            }
        }
        Operation::RemoveConstraint { model, name } => remove_constraint(editor, context, model, name),
        Operation::CreateModel(_)
        | Operation::DeleteModel { .. }
        | Operation::AddField { .. }
        | Operation::RemoveField { .. }
        | Operation::AlterField { .. }
        | Operation::RenameField { .. }
        | Operation::RenameModel { .. }
        | Operation::AlterModelTable { .. }
        | Operation::AlterManaged { .. }
        | Operation::RunSql(_)
        | Operation::CreateExtension(_)
        | Operation::Statement { .. }
        | Operation::Separate { .. } => {}
    }
}

fn alter_unique_together(editor: &mut Editor, context: Context<'_>, model: &str, sets: &[Vec<String>]) {
    let (Some(old_model), Some(new_model)) = (context.old_model(model), context.new_model(model)) else {
        return;
    };
    let table = new_model.table();
    for set in old_model.unique_together.iter().filter(|set| !sets.contains(set)) {
        let columns = columns_of(old_model, set);
        let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
        editor.execute(&format!(
            "ALTER TABLE {} DROP CONSTRAINT {}",
            quoted(&old_model.table()),
            quoted(&unique_together_name(&old_model.table(), &columns))
        ));
    }
    for set in sets.iter().filter(|set| !old_model.unique_together.contains(set)) {
        editor.run(&Statement::on_columns(
            &table,
            &columns_of(new_model, set),
            Kind::Unique,
        ));
    }
}

fn unnamed_index_name(owner: &Model, fields: &[String]) -> String {
    let columns = columns_of(owner, fields);
    let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
    let single_indexed_field = match fields {
        [field] => owner.field(field).is_some_and(super::model::Field::db_index),
        _ => false,
    };
    let suffix = if single_indexed_field {
        ""
    } else {
        INDEX_TOGETHER_SUFFIX
    };
    index_name(&owner.table(), &columns, suffix)
}

fn add_constraint(editor: &mut Editor, owner: &Model, constraint: &Constraint, not_valid: bool) {
    let validity = if not_valid { NOT_VALID } else { "" };
    match constraint_definition(owner, constraint) {
        Some(definition) => editor.execute(&format!(
            "ALTER TABLE {} ADD CONSTRAINT {} {definition}{validity}",
            quoted(&owner.table()),
            quoted(&constraint.name)
        )),
        None => editor.run(&unique_index(owner, constraint)),
    }
}

fn remove_constraint(editor: &mut Editor, context: Context<'_>, model: &str, name: &str) {
    let Some(owner) = context.old_model(model) else {
        return;
    };
    let Some(constraint) = owner
        .constraints
        .iter()
        .find(|constraint| constraint.name == name)
    else {
        return;
    };
    let quoted_name = quoted(name);
    if matches!(constraint.kind, ConstraintKind::Unique { by_index: true, .. }) {
        editor.execute(&format!("DROP INDEX IF EXISTS {quoted_name}"));
    } else {
        editor.execute(&format!(
            "ALTER TABLE {} DROP CONSTRAINT {quoted_name}",
            quoted(&owner.table())
        ));
    }
}
