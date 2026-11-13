use std::borrow::Cow;
use std::collections::BTreeMap;

use super::dependencies::SWAPPABLE_SETTING_APP;
use super::model::{Class, Field, Flag, Index, Model, ModelKey, Relation};
use super::names::truncated_table;
use super::operations::{Deletion, Operation};

const ID: &str = "id";
const FROM_PREFIX: &str = "from_";
const TO_PREFIX: &str = "to_";
const BUILT_IN_TABLES: &[(&str, &str, &str)] = &[
    ("auth", "user", "auth_user"),
    ("auth", "group", "auth_group"),
    ("auth", "permission", "auth_permission"),
    ("contenttypes", "contenttype", "django_content_type"),
    ("sites", "site", "django_site"),
    ("taggit", "tag", "taggit_tag"),
];

#[derive(Debug, Clone)]
pub struct State {
    models: BTreeMap<ModelKey, Model>,
    auto_field: Class,
    column_drops_cascade: bool,
    declared_swappables: Vec<(String, ModelKey)>,
    pending_models: BTreeMap<ModelKey, Model>,
    pending_fields: BTreeMap<(ModelKey, String), Model>,
}

impl State {
    pub const fn new(auto_field: Class, column_drops_cascade: bool) -> Self {
        Self {
            models: BTreeMap::new(),
            auto_field,
            column_drops_cascade,
            declared_swappables: Vec::new(),
            pending_models: BTreeMap::new(),
            pending_fields: BTreeMap::new(),
        }
    }

    pub fn with_declared_swappables(mut self, declared: Vec<(String, ModelKey)>) -> Self {
        self.declared_swappables = declared;
        self
    }

    pub const fn column_drops_cascade(&self) -> bool {
        self.column_drops_cascade
    }

    pub fn model(&self, key: &ModelKey) -> Option<&Model> {
        self.models.get(key)
    }

    pub fn pending_model(&self, key: &ModelKey) -> Option<&Model> {
        self.pending_models.get(key)
    }

    pub fn pending_field_owner(&self, key: &ModelKey, field: &str) -> Option<&Model> {
        self.pending_fields.get(&(key.clone(), field.to_owned()))
    }

    pub fn models(&self) -> impl Iterator<Item = &Model> {
        self.models.values()
    }

    pub fn resolve(&self, key: &ModelKey) -> Cow<'_, Model> {
        self.models
            .get(key)
            .or_else(|| self.swappable(key))
            .map_or_else(|| Cow::Owned(fallback(key, self.auto_field)), Cow::Borrowed)
    }

    fn swappable(&self, key: &ModelKey) -> Option<&Model> {
        if key.app != SWAPPABLE_SETTING_APP {
            return None;
        }
        self.models
            .values()
            .find(|model| {
                model
                    .swappable_setting
                    .as_ref()
                    .is_some_and(|setting| key.model.ends_with(setting.as_str()))
            })
            .or_else(|| {
                self.declared_swappables
                    .iter()
                    .find(|(setting, _)| key.model.ends_with(setting.as_str()))
                    .and_then(|(_, declaring)| self.models.get(declaring))
            })
    }

    pub fn through(&self, owner: &Model, field: &Field) -> Option<Model> {
        let relation = field.relation.as_ref()?;
        let target = self.resolve(&relation.to);
        let (mut from, mut to) = (owner.key.model.clone(), target.key.model.clone());
        if from == to {
            from = format!("{FROM_PREFIX}{from}");
            to = format!("{TO_PREFIX}{to}");
        }
        let mut through = Model::new(ModelKey::new(
            &owner.key.app,
            &format!("{}_{}", owner.key.model, field.name),
        ));
        through.db_table = Some(
            relation
                .db_table
                .clone()
                .unwrap_or_else(|| truncated_table(&format!("{}_{}", owner.table(), field.name))),
        );
        let constrained = field.db_constraint();
        through.fields = vec![
            identity(self.auto_field),
            link(&from, owner.key.clone(), constrained),
            link(&to, relation.to.clone(), constrained),
        ];
        through.unique_together = vec![vec![from, to]];
        Some(through)
    }

    pub fn apply(&mut self, app: &str, operation: &Operation) {
        let key = |model: &str| ModelKey::new(app, model);
        match operation {
            Operation::CreateModel(model) => {
                self.models.insert(model.key.clone(), model.clone());
            }
            Operation::DeleteModel { name, deletion } => self.delete_model(key(name), *deletion),
            Operation::RemoveField {
                model,
                name,
                deletion: deletion @ (Deletion::MoveToPending | Deletion::Purge),
            } => self.remove_field_safely(key(model), name, *deletion),
            Operation::RenameModel { old, new } => self.rename_model(&key(old), &key(new)),
            Operation::Separate { state, .. } => {
                for located in state {
                    self.apply(app, &located.operation);
                }
            }
            Operation::RunSql(_) | Operation::CreateExtension(_) | Operation::Statement { .. } => {}
            Operation::AddField { model, .. }
            | Operation::RemoveField { model, .. }
            | Operation::AlterField { model, .. }
            | Operation::RenameField { model, .. }
            | Operation::AlterModelTable { model, .. }
            | Operation::AlterUniqueTogether { model, .. }
            | Operation::AlterManaged { model, .. }
            | Operation::AddIndex { model, .. }
            | Operation::RemoveIndex { model, .. }
            | Operation::RenameIndex { model, .. }
            | Operation::AddConstraint { model, .. }
            | Operation::RemoveConstraint { model, .. } => {
                if let Some(target) = self.models.get_mut(&key(model)) {
                    change(target, operation);
                }
            }
        }
    }

    fn delete_model(&mut self, key: ModelKey, deletion: Deletion) {
        if deletion == Deletion::Purge {
            self.pending_models.remove(&key);
            return;
        }
        let removed = self.models.remove(&key);
        if let (Deletion::MoveToPending, Some(model)) = (deletion, removed) {
            self.pending_models.insert(key, model);
        }
    }

    fn remove_field_safely(&mut self, key: ModelKey, name: &str, deletion: Deletion) {
        let pending_key = (key, name.to_owned());
        if deletion == Deletion::Purge {
            self.pending_fields.remove(&pending_key);
            return;
        }
        let Some(owner) = self.models.get_mut(&pending_key.0) else {
            return;
        };
        let snapshot = owner.clone();
        owner.fields.retain(|field| field.name != name);
        self.pending_fields.insert(pending_key, snapshot);
    }

    fn rename_model(&mut self, old: &ModelKey, new: &ModelKey) {
        let Some(mut model) = self.models.remove(old) else {
            return;
        };
        model.key = new.clone();
        self.models.insert(new.clone(), model);
        for relation in self
            .models
            .values_mut()
            .flat_map(|other| other.fields.iter_mut())
            .filter_map(|field| field.relation.as_mut())
            .filter(|relation| relation.to == *old)
        {
            relation.to = new.clone();
        }
    }
}

fn change(model: &mut Model, operation: &Operation) {
    match operation {
        Operation::AddField { field, .. } => model.fields.push(field.clone()),
        Operation::RemoveField { name, .. } => model.fields.retain(|field| field.name != *name),
        Operation::AlterField { field, .. } => {
            if let Some(existing) = model
                .fields
                .iter_mut()
                .find(|existing| existing.name == field.name)
            {
                *existing = field.clone();
            }
        }
        Operation::RenameField { old, new, .. } => rename_field(model, old, new),
        Operation::AlterModelTable { table, .. } => model.db_table.clone_from(table),
        Operation::AlterUniqueTogether { sets, .. } => model.unique_together.clone_from(sets),
        Operation::AlterManaged { managed, .. } => model.managed = *managed,
        Operation::AddIndex { index, .. } => model.indexes.push(index.clone()),
        Operation::RemoveIndex { name, .. } => model.indexes.retain(|index| index.name != *name),
        Operation::RenameIndex {
            old: None,
            old_fields,
            new,
            ..
        } => model.indexes.push(Index {
            name: new.clone(),
            fields: old_fields.clone(),
            expressions: Vec::new(),
            include: Vec::new(),
            condition: None,
            method: None,
            opclasses: Vec::new(),
            storage: Vec::new(),
        }),
        Operation::RenameIndex { old, new, .. } => {
            if let Some(index) = model
                .indexes
                .iter_mut()
                .find(|index| Some(&index.name) == old.as_ref())
            {
                index.name.clone_from(new);
            }
        }
        Operation::AddConstraint { constraint, .. } => model.constraints.push(constraint.clone()),
        Operation::RemoveConstraint { name, .. } => {
            model.constraints.retain(|constraint| constraint.name != *name);
        }
        Operation::CreateModel(_)
        | Operation::DeleteModel { .. }
        | Operation::RenameModel { .. }
        | Operation::RunSql(_)
        | Operation::CreateExtension(_)
        | Operation::Statement { .. }
        | Operation::Separate { .. } => {}
    }
}

fn rename_field(model: &mut Model, old: &str, new: &str) {
    let renamed = |name: &mut String| {
        if name == old {
            new.clone_into(name);
        }
    };
    if let Some(field) = model.fields.iter_mut().find(|field| field.name == old) {
        new.clone_into(&mut field.name);
    }
    model.unique_together.iter_mut().flatten().for_each(renamed);
    for index in &mut model.indexes {
        index.fields.iter_mut().for_each(|field| {
            let descending = field.strip_prefix('-').is_some_and(|name| name == old);
            if descending {
                *field = format!("-{new}");
            } else {
                renamed(field);
            }
        });
    }
}

fn identity(class: Class) -> Field {
    let mut field = Field::new(ID, class);
    field.flags.insert(Flag::PrimaryKey);
    field
}

fn link(name: &str, to: ModelKey, constrained: bool) -> Field {
    let mut field = Field::new(name, Class::ForeignKey);
    if !constrained {
        field.flags.insert(Flag::NoConstraint);
    }
    field.relation = Some(Relation {
        to,
        to_field: None,
        through: None,
        db_table: None,
    });
    field
}

fn fallback(key: &ModelKey, auto_field: Class) -> Model {
    let built_in = BUILT_IN_TABLES
        .iter()
        .find(|(app, model, _)| *app == key.app && *model == key.model);
    let mut model = Model::new(key.clone());
    model.db_table = built_in.map(|(_, _, table)| (*table).to_owned());
    model.fields = vec![identity(if built_in.is_some() {
        Class::Auto
    } else {
        auto_field
    })];
    model
}
