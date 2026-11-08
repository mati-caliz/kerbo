use super::alter::{FieldChange, alter_field as alter_column};
use super::deferred::{IndexKind, Kind, Statement};
use super::model::{Constraint, ConstraintKind, Model, ModelKey};
use super::names::quoted;
use super::operations::{Located, Migration, Operation};
use super::schema::{Editor, columns_of, named_index, unique_index};
use super::state::State;
use crate::deploy::Violation;
use crate::deploy::script::Origin;

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
    let mut editor = Editor::new(migration.atomic);
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
        Operation::DeleteModel(name) => {
            if let Some(deleted) = context.old_model(name) {
                editor.delete_model(deleted, context.before);
            }
        }
        Operation::RunSql(chunks) => {
            for chunk in chunks {
                editor.execute_raw(&chunk.sql, chunk.line);
            }
        }
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
    let (Some(old_model), Some(new_model)) = (context.old_model(old), context.new_model(new)) else {
        return;
    };
    editor.rename_table(&old_model.table(), &new_model.table());
    for field in old_model.fields.iter().filter(|field| field.auto_through()) {
        let old_through = context.before.through(old_model, field);
        let new_through = new_model
            .field(&field.name)
            .and_then(|new_field| context.after.through(new_model, new_field));
        if let (Some(old_through), Some(new_through)) = (old_through, new_through) {
            editor.rename_table(&old_through.table(), &new_through.table());
        }
    }
}

fn render_field(editor: &mut Editor, context: Context<'_>, operation: &Operation) {
    match operation {
        Operation::AddField { model, field } => {
            if let Some(owner) = context.new_model(model) {
                editor.add_field(owner, field, context.after);
            }
        }
        Operation::RemoveField { model, name } => {
            if let Some(owner) = context.old_model(model)
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
        | Operation::DeleteModel(_)
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
    let table = new_model.table();
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
        Operation::RenameIndex { model, old, new } => {
            if context.new_model(model).is_some()
                && let Some(old) = old
            {
                editor.execute(&format!("ALTER INDEX {} RENAME TO {}", quoted(old), quoted(new)));
            }
        }
        Operation::AddConstraint { model, constraint } => {
            if let Some(owner) = context.new_model(model) {
                add_constraint(editor, owner, constraint);
            }
        }
        Operation::RemoveConstraint { model, name } => remove_constraint(editor, context, model, name),
        Operation::CreateModel(_)
        | Operation::DeleteModel(_)
        | Operation::AddField { .. }
        | Operation::RemoveField { .. }
        | Operation::AlterField { .. }
        | Operation::RenameField { .. }
        | Operation::RenameModel { .. }
        | Operation::AlterModelTable { .. }
        | Operation::AlterManaged { .. }
        | Operation::RunSql(_)
        | Operation::Separate { .. } => {}
    }
}

fn alter_unique_together(editor: &mut Editor, context: Context<'_>, model: &str, sets: &[Vec<String>]) {
    let (Some(old_model), Some(new_model)) = (context.old_model(model), context.new_model(model)) else {
        return;
    };
    let table = new_model.table();
    for set in sets.iter().filter(|set| !old_model.unique_together.contains(set)) {
        editor.run(&Statement::on_columns(
            &table,
            &columns_of(new_model, set),
            Kind::Unique,
        ));
    }
}

fn add_constraint(editor: &mut Editor, owner: &Model, constraint: &Constraint) {
    let table = quoted(&owner.table());
    let name = quoted(&constraint.name);
    match &constraint.kind {
        ConstraintKind::Unique { by_index: true, .. } => editor.run(&unique_index(owner, constraint)),
        ConstraintKind::Unique { fields, .. } => {
            let columns = columns_of(owner, fields)
                .iter()
                .map(|column| quoted(column))
                .collect::<Vec<_>>()
                .join(", ");
            editor.execute(&format!(
                "ALTER TABLE {table} ADD CONSTRAINT {name} UNIQUE ({columns})"
            ));
        }
        ConstraintKind::Check => {
            editor.execute(&format!("ALTER TABLE {table} ADD CONSTRAINT {name} CHECK (true)"));
        }
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
