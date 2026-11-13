use super::alter::{FieldChange, alter_field};
use super::deferred::{Kind, Statement, foreign_key_name};
use super::model::{Field, Model};
use super::names::quoted;
use super::schema::{Editor, constrained_target};
use super::state::State;

pub struct Side<'side> {
    pub owner: &'side Model,
    pub field: &'side Field,
    pub state: &'side State,
}

pub fn alter_many_to_many(editor: &mut Editor, old: &Side<'_>, new: &Side<'_>) {
    if !(old.field.auto_through() && new.field.auto_through()) {
        return;
    }
    let (Some(old_through), Some(new_through)) = (
        old.state.through(old.owner, old.field),
        new.state.through(new.owner, new.field),
    ) else {
        return;
    };
    let table = new_through.table();
    editor.rename_table(&old_through.table(), &table);
    for (old_link, new_link) in old_through.fields.iter().zip(&new_through.fields).rev() {
        if old_link == new_link || !old_link.is_foreign_key() {
            continue;
        }
        repoint(editor, &table, (old_link, new_link), (old.state, new.state));
    }
}

fn repoint(
    editor: &mut Editor,
    table: &str,
    (old, new): (&Field, &Field),
    (before, after): (&State, &State),
) {
    if let Some((to_table, to_column)) = constrained_target(old, before) {
        let name = quoted(&foreign_key_name(table, &[&old.column()], &to_table, &to_column));
        editor.execute(&format!(
            "SET CONSTRAINTS {name} IMMEDIATE; ALTER TABLE {} DROP CONSTRAINT {name}",
            quoted(table)
        ));
    }
    alter_field(
        editor,
        FieldChange {
            table,
            old,
            new,
            before,
            after,
        },
    );
    if let Some((to_table, to_column)) = constrained_target(new, after) {
        editor.execute(
            &Statement::on_columns(table, &[new.column()], Kind::ForeignKey { to_table, to_column }).render(),
        );
    }
}
