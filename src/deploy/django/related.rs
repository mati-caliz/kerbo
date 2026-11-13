use super::columns::{db_type, type_template};
use super::deferred::foreign_key_name;
use super::model::{Field, Model, ModelKey};
use super::names::{index_name, quoted};
use super::schema::Editor;
use super::state::State;

const REBUILT_FOREIGN_KEY_SUFFIX: &str = "_fk";

pub struct Incoming {
    table: String,
    column: String,
    old: Field,
    new: Field,
}

pub fn incoming(target: &ModelKey, before: &State, after: &State) -> Vec<Incoming> {
    let mut found = Vec::new();
    for model in after.models().filter(|model| model.emits_sql()) {
        let Some(old_model) = before.model(&model.key) else {
            continue;
        };
        collect(&mut found, (model, old_model), target, after);
        for field in model.fields.iter().filter(|field| field.auto_through()) {
            let old_field = old_model.field(&field.name);
            let throughs = after
                .through(model, field)
                .zip(old_field.and_then(|old_field| before.through(old_model, old_field)));
            if let Some((through, old_through)) = throughs {
                collect(&mut found, (&through, &old_through), target, after);
            }
        }
    }
    found
}

fn collect(
    found: &mut Vec<Incoming>,
    (model, old_model): (&Model, &Model),
    target: &ModelKey,
    state: &State,
) {
    let pointing = model.local_fields().filter(|field| {
        field.is_foreign_key()
            && field.db_constraint()
            && field.relation.as_ref().is_some_and(|relation| {
                relation.to_field.is_none() && state.resolve(&relation.to).key == *target
            })
    });
    for field in pointing {
        if let Some(old) = old_model.field(&field.name) {
            found.push(Incoming {
                table: model.table(),
                column: field.column(),
                old: old.clone(),
                new: field.clone(),
            });
        }
    }
}

pub fn drop_constraints(
    editor: &mut Editor,
    incoming: &[Incoming],
    (target_table, target_column): (&str, &str),
) {
    for rel in incoming {
        let name = quoted(&foreign_key_name(
            &rel.table,
            &[&rel.column],
            target_table,
            target_column,
        ));
        editor.execute(&format!(
            "SET CONSTRAINTS {name} IMMEDIATE; ALTER TABLE {} DROP CONSTRAINT {name}",
            quoted(&rel.table)
        ));
    }
}

pub fn alter_types(editor: &mut Editor, incoming: &[Incoming], before: &State, after: &State) {
    for rel in incoming {
        let column = quoted(&rel.column);
        let new_type = db_type(&rel.new, after);
        let using = if type_template(&rel.old, before) == type_template(&rel.new, after) {
            String::new()
        } else {
            format!(" USING {column}::{new_type}")
        };
        editor.execute(&format!(
            "ALTER TABLE {} ALTER COLUMN {column} TYPE {new_type}{using}",
            quoted(&rel.table)
        ));
    }
}

pub fn rebuild_constraints(
    editor: &mut Editor,
    incoming: &[Incoming],
    (target_table, target_column): (&str, &str),
) {
    for rel in incoming {
        editor.execute(&format!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) DEFERRABLE INITIALLY DEFERRED",
            quoted(&rel.table),
            quoted(&index_name(&rel.table, &[&rel.column], REBUILT_FOREIGN_KEY_SUFFIX)),
            quoted(&rel.column),
            quoted(target_table),
            quoted(target_column)
        ));
    }
}
