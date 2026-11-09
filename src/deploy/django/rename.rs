use super::model::{Field, Model};
use super::schema::Editor;
use super::state::State;

const THROUGH_LINKS_AFTER_IDENTITY: usize = 1;

pub fn rename_model(
    editor: &mut Editor,
    before: &State,
    after: &State,
    (old_model, new_model): (&Model, &Model),
) {
    editor.rename_table(&old_model.table(), &new_model.table());
    for owner in after.models().filter(|owner| owner.emits_sql()) {
        let old_owner = if owner.key == new_model.key {
            Some(old_model)
        } else {
            before.model(&owner.key)
        };
        let Some(old_owner) = old_owner else {
            continue;
        };
        let affected = owner.fields.iter().filter(|field| {
            field.auto_through() && (owner.key == new_model.key || targets(field, new_model))
        });
        for field in affected {
            let old_through = old_owner
                .field(&field.name)
                .and_then(|old_field| before.through(old_owner, old_field));
            if let (Some(old_through), Some(new_through)) = (old_through, after.through(owner, field)) {
                rename_through(editor, &old_through, &new_through);
            }
        }
    }
}

fn targets(field: &Field, model: &Model) -> bool {
    field
        .relation
        .as_ref()
        .is_some_and(|relation| relation.to == model.key)
}

fn rename_through(editor: &mut Editor, old: &Model, new: &Model) {
    let table = new.table();
    editor.rename_table(&old.table(), &table);
    let links = old
        .fields
        .iter()
        .zip(&new.fields)
        .skip(THROUGH_LINKS_AFTER_IDENTITY)
        .rev();
    for (old_link, new_link) in links {
        let (old_column, new_column) = (old_link.column(), new_link.column());
        if old_column != new_column {
            editor.rename_column(&table, &old_column, &new_column);
        }
    }
}
