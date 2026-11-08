use std::borrow::Cow;
use std::collections::BTreeMap;

use super::model::{Class, Field, Flag, Model, ModelKey, Relation};
use super::names::truncated_table;
use super::operations::Operation;

const ID: &str = "id";
const FROM_PREFIX: &str = "from_";
const TO_PREFIX: &str = "to_";
const BUILT_IN_TABLES: &[(&str, &str, &str)] = &[
    ("auth", "user", "auth_user"),
    ("auth", "group", "auth_group"),
    ("auth", "permission", "auth_permission"),
    ("contenttypes", "contenttype", "django_content_type"),
    ("sites", "site", "django_site"),
];

#[derive(Debug, Clone)]
pub struct State {
    models: BTreeMap<ModelKey, Model>,
    auto_field: Class,
}

impl State {
    pub const fn new(auto_field: Class) -> Self {
        Self {
            models: BTreeMap::new(),
            auto_field,
        }
    }

    pub fn model(&self, key: &ModelKey) -> Option<&Model> {
        self.models.get(key)
    }

    pub fn contains(&self, key: &ModelKey) -> bool {
        self.models.contains_key(key)
    }

    pub fn related_keys(&self) -> Vec<ModelKey> {
        self.models
            .values()
            .flat_map(|model| model.fields.iter())
            .filter_map(|field| field.relation.as_ref())
            .map(|relation| relation.to.clone())
            .collect()
    }

    pub fn resolve(&self, key: &ModelKey) -> Cow<'_, Model> {
        self.models
            .get(key)
            .map_or_else(|| Cow::Owned(fallback(key, self.auto_field)), Cow::Borrowed)
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
            Operation::DeleteModel(name) => {
                self.models.remove(&key(name));
            }
            Operation::RenameModel { old, new } => self.rename_model(&key(old), &key(new)),
            Operation::Separate { state, .. } => {
                for located in state {
                    self.apply(app, &located.operation);
                }
            }
            Operation::RunSql(_) | Operation::CreateExtension(_) => {}
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
        | Operation::DeleteModel(_)
        | Operation::RenameModel { .. }
        | Operation::RunSql(_)
        | Operation::CreateExtension(_)
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
