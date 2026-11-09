use std::path::Path;
use std::sync::Arc;

use tree_sitter::Node;

mod raw_sql;

use super::constants::{Constants, assignments, class_attributes};
use super::dependencies::keys;
use super::factories::{Factory, imported};
use super::fields::{Owner, field};
use super::meta::{constraint, constraints, index, indexes, unique_together};
use super::model::{Model, ModelKey};
use super::operations::{Located, Migration, Operation};
use super::run_python::functions;
use super::settings::Settings;
use super::truth::truth;
use super::values::{boolean, dictionary, elements, text};
use crate::deploy::python_calls::Call;
use crate::syntax::named_children;
use raw_sql::{run_python, run_sql};

const MIGRATION_CLASS: &str = "Migration";
const OPERATIONS: &str = "operations";
const ATOMIC: &str = "atomic";
const DEPENDENCIES: &str = "dependencies";
const RUN_BEFORE: &str = "run_before";
const CONCURRENTLY: &str = "Concurrently";
const ADD_FIELD: &str = "AddField";
const CONCATENATION: &str = "binary_operator";
const RUN_PYTHON: &str = "RunPython";
const GENERIC_EXTENSION: &str = "CreateExtension";
const NAMED_EXTENSIONS: &[(&str, &str)] = &[
    ("BloomExtension", "bloom"),
    ("BtreeGinExtension", "btree_gin"),
    ("BtreeGistExtension", "btree_gist"),
    ("CITextExtension", "citext"),
    ("CryptoExtension", "pgcrypto"),
    ("HStoreExtension", "hstore"),
    ("TrigramExtension", "pg_trgm"),
    ("UnaccentExtension", "unaccent"),
];

#[derive(Debug, Clone, Copy)]
pub struct Reader<'context> {
    pub source: &'context [u8],
    pub app: &'context str,
    pub settings: &'context Settings,
    pub apps_root: Option<&'context Path>,
}

#[derive(Debug, Clone, Copy)]
struct Scope<'context> {
    source: &'context [u8],
    app: &'context str,
    settings: &'context Settings,
    constants: Constants<'context>,
    functions: &'context [(String, Node<'context>)],
    factories: &'context [(String, Arc<Factory>)],
}

pub fn migration(root: Node<'_>, reader: Reader<'_>) -> Option<Migration> {
    let class = named_children(root).into_iter().find(|node| {
        node.kind() == "class_definition"
            && node
                .child_by_field_name("name")
                .and_then(|name| name.utf8_text(reader.source).ok())
                == Some(MIGRATION_CLASS)
    })?;
    let body = class.child_by_field_name("body")?;
    let class_bindings = assignments(body, reader.source);
    let mut bindings = class_attributes(root, reader.source);
    bindings.extend(assignments(root, reader.source));
    bindings.extend(class_bindings.iter().cloned());
    let module_functions = functions(root, reader.source);
    let factories = reader
        .apps_root
        .map(|apps_root| imported(root, reader.source, apps_root, reader.settings))
        .unwrap_or_default();
    let scope = Scope {
        source: reader.source,
        app: reader.app,
        settings: reader.settings,
        constants: Constants::new(reader.source, &bindings),
        functions: &module_functions,
        factories: &factories,
    };
    let mut migration = Migration {
        operations: Vec::new(),
        atomic: true,
        dependencies: Vec::new(),
        run_before: Vec::new(),
    };
    for (name, value) in class_bindings {
        match name.as_str() {
            OPERATIONS => migration.operations = scope.operations(value),
            ATOMIC => {
                migration.atomic =
                    truth(scope.resolved(value), reader.source, reader.settings) != Some(false);
            }
            DEPENDENCIES => {
                migration.dependencies = keys(scope.resolved(value), reader.source, reader.settings);
            }
            RUN_BEFORE => migration.run_before = keys(scope.resolved(value), reader.source, reader.settings),
            _ => {}
        }
    }
    Some(migration)
}

fn argument<'tree>(call: &Call<'tree>, index: usize, name: &str) -> Option<Node<'tree>> {
    call.keyword(name).or_else(|| call.positional(index))
}

fn string_argument(call: &Call<'_>, index: usize, name: &str) -> Option<String> {
    argument(call, index, name).and_then(|node| text(node, call.source))
}

fn model_name(call: &Call<'_>, index: usize, name: &str) -> Option<String> {
    string_argument(call, index, name).map(|model| model.to_lowercase())
}

impl<'context> Scope<'context> {
    fn resolved(self, node: Node<'context>) -> Node<'context> {
        self.constants.resolve(node)
    }

    fn sequence(self, node: Node<'context>) -> Vec<Node<'context>> {
        let node = self.resolved(node);
        if node.kind() == CONCATENATION
            && let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            )
        {
            let mut joined = self.sequence(left);
            joined.extend(self.sequence(right));
            return joined;
        }
        elements(node)
    }

    fn operations(self, node: Node<'context>) -> Vec<Located> {
        self.located(node, |call| self.operation(call))
    }

    fn database_operations(self, node: Node<'context>) -> Vec<Located> {
        self.located(node, |call| {
            if call.method == RUN_PYTHON {
                run_python(self, call)
            } else {
                self.operation(call)
            }
        })
    }

    fn located(
        self,
        node: Node<'context>,
        read: impl Fn(&Call<'context>) -> Option<Operation>,
    ) -> Vec<Located> {
        self.sequence(node)
            .into_iter()
            .filter_map(|element| {
                let call = Call::parse(self.resolved(element), self.source)?;
                Some(Located {
                    operation: read(&call)?,
                    line: call.line,
                    column: call.column,
                })
            })
            .collect()
    }

    fn operation(self, call: &Call<'context>) -> Option<Operation> {
        self.declared_operation(call)
            .or_else(|| self.factory_operation(call))
    }

    fn factory_operation(self, call: &Call<'context>) -> Option<Operation> {
        if !call.receiver.is_empty() {
            return None;
        }
        let factory = self
            .factories
            .iter()
            .rev()
            .find(|(name, _)| name == call.method)
            .map(|(_, factory)| factory)?;
        let arguments = factory
            .forwarded
            .iter()
            .map(|parameter| argument(call, parameter.position, &parameter.name))
            .collect::<Option<Vec<_>>>()?;
        self.declared_operation(&Call {
            receiver: "",
            method: &factory.method,
            line: call.line,
            column: call.column,
            arguments,
            source: call.source,
        })
    }

    fn declared_operation(self, call: &Call<'context>) -> Option<Operation> {
        match call.method {
            "CreateModel" => self.create_model(call),
            "DeleteModel" => model_name(call, 0, "name").map(Operation::DeleteModel),
            "AddField" | "AlterField" => self.field_operation(call),
            "RemoveField" => Some(Operation::RemoveField {
                model: model_name(call, 0, "model_name")?,
                name: string_argument(call, 1, "name")?,
            }),
            "RenameField" => Some(Operation::RenameField {
                model: model_name(call, 0, "model_name")?,
                old: string_argument(call, 1, "old_name")?,
                new: string_argument(call, 2, "new_name")?,
            }),
            "RenameModel" => Some(Operation::RenameModel {
                old: model_name(call, 0, "old_name")?,
                new: model_name(call, 1, "new_name")?,
            }),
            "AlterModelTable" => Some(Operation::AlterModelTable {
                model: model_name(call, 0, "name")?,
                table: string_argument(call, 1, "table"),
            }),
            "AlterUniqueTogether" => Some(Operation::AlterUniqueTogether {
                model: model_name(call, 0, "name")?,
                sets: argument(call, 1, "unique_together")
                    .map(|node| unique_together(node, self.source))
                    .unwrap_or_default(),
            }),
            "AlterModelOptions" => self.alter_options(call),
            "RunSQL" => run_sql(self, call),
            GENERIC_EXTENSION => string_argument(call, 0, "name").map(Operation::CreateExtension),
            "SeparateDatabaseAndState" => Some(Operation::Separate {
                database: argument(call, 0, "database_operations")
                    .map(|node| self.database_operations(node))
                    .unwrap_or_default(),
                state: argument(call, 1, "state_operations")
                    .map(|node| self.operations(node))
                    .unwrap_or_default(),
            }),
            method => NAMED_EXTENSIONS
                .iter()
                .find(|(class, _)| *class == method)
                .map(|(_, name)| Operation::CreateExtension((*name).to_owned()))
                .or_else(|| self.index_operation(call)),
        }
    }

    fn index_operation(self, call: &Call<'_>) -> Option<Operation> {
        let concurrently = call.method.ends_with(CONCURRENTLY);
        let model = model_name(call, 0, "model_name")?;
        match call.method.trim_end_matches(CONCURRENTLY) {
            "AddIndex" => Some(Operation::AddIndex {
                model,
                index: argument(call, 1, "index").and_then(|node| index(node, self.constants))?,
                concurrently,
            }),
            "RemoveIndex" => Some(Operation::RemoveIndex {
                model,
                name: string_argument(call, 1, "name")?,
                concurrently,
            }),
            "RenameIndex" => Some(Operation::RenameIndex {
                model,
                new: string_argument(call, 1, "new_name")?,
                old: call.keyword("old_name").and_then(|node| text(node, self.source)),
            }),
            "AddConstraint" => Some(Operation::AddConstraint {
                model,
                constraint: argument(call, 1, "constraint")
                    .and_then(|node| constraint(node, self.constants))?,
            }),
            "RemoveConstraint" => Some(Operation::RemoveConstraint {
                model,
                name: string_argument(call, 1, "name")?,
            }),
            _ => None,
        }
    }

    fn field_operation(self, call: &Call<'_>) -> Option<Operation> {
        let model = model_name(call, 0, "model_name")?;
        let name = string_argument(call, 1, "name")?;
        let owner = Owner {
            app: self.app,
            model: &model,
            settings: self.settings,
            constants: self.constants,
        };
        let parsed = field(&name, argument(call, 2, "field")?, self.source, owner)?;
        Some(if call.method == ADD_FIELD {
            Operation::AddField { model, field: parsed }
        } else {
            Operation::AlterField { model, field: parsed }
        })
    }

    fn create_model(self, call: &Call<'_>) -> Option<Operation> {
        let name = model_name(call, 0, "name")?;
        let mut model = Model::new(ModelKey::new(self.app, &name));
        let owner = Owner {
            app: self.app,
            model: &name,
            settings: self.settings,
            constants: self.constants,
        };
        model.fields = argument(call, 1, "fields")
            .map(elements)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pair| {
                let parts = elements(pair);
                let field_name = text(*parts.first()?, self.source)?;
                field(&field_name, *parts.get(1)?, self.source, owner)
            })
            .collect();
        if let Some(options) = argument(call, 2, "options") {
            self.apply_options(&mut model, options);
        }
        Some(Operation::CreateModel(model))
    }

    fn apply_options(self, model: &mut Model, options: Node<'_>) {
        for (key, value) in dictionary(options, self.source) {
            match key.as_str() {
                "db_table" => model.db_table = text(value, self.source),
                "unique_together" => model.unique_together = unique_together(value, self.source),
                "indexes" => model.indexes = indexes(value, self.constants),
                "constraints" => model.constraints = constraints(value, self.constants),
                "managed" => model.managed = boolean(value) != Some(false),
                "proxy" => model.proxy = boolean(value) == Some(true),
                "swappable" => {
                    model.swapped_for = text(value, self.source)
                        .and_then(|setting| self.settings.replacement(&setting, &model.key));
                }
                _ => {}
            }
        }
    }

    fn alter_options(self, call: &Call<'_>) -> Option<Operation> {
        let model = model_name(call, 0, "name")?;
        let managed = dictionary(argument(call, 1, "options")?, self.source)
            .into_iter()
            .find(|(key, _)| key == "managed")
            .and_then(|(_, value)| boolean(value))?;
        Some(Operation::AlterManaged { model, managed })
    }
}
