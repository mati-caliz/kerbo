use tree_sitter::Node;

use super::fields::{Owner, field};
use super::meta::{constraint, constraints, index, indexes, unique_together};
use super::model::{Model, ModelKey};
use super::operations::{Located, Migration, Operation, SqlChunk};
use super::settings::Settings;
use super::values::{boolean, dictionary, elements, is_sequence, text};
use crate::deploy::python_calls::Call;
use crate::syntax::{named_children, one_based};

const MIGRATION_CLASS: &str = "Migration";
const OPERATIONS: &str = "operations";
const ATOMIC: &str = "atomic";
const NOOP: &str = "noop";
const CONCURRENTLY: &str = "Concurrently";
const ADD_FIELD: &str = "AddField";
const IDENTIFIER: &str = "identifier";
const CONCATENATION: &str = "binary_operator";
const MAX_ALIAS_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Reader<'context> {
    pub source: &'context [u8],
    pub app: &'context str,
    pub settings: &'context Settings,
}

#[derive(Debug, Clone, Copy)]
struct Scope<'context> {
    source: &'context [u8],
    app: &'context str,
    settings: &'context Settings,
    bindings: &'context [(String, Node<'context>)],
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
    let mut bindings = assignments(root, reader.source);
    bindings.extend(class_bindings.iter().cloned());
    let scope = Scope {
        source: reader.source,
        app: reader.app,
        settings: reader.settings,
        bindings: &bindings,
    };
    let mut migration = Migration {
        operations: Vec::new(),
        atomic: true,
    };
    for (name, value) in class_bindings {
        match name.as_str() {
            OPERATIONS => migration.operations = scope.operations(value),
            ATOMIC => migration.atomic = boolean(value) != Some(false),
            _ => {}
        }
    }
    Some(migration)
}

fn assignments<'tree>(body: Node<'tree>, source: &[u8]) -> Vec<(String, Node<'tree>)> {
    named_children(body)
        .into_iter()
        .filter_map(|statement| named_children(statement).into_iter().next())
        .filter(|assignment| assignment.kind() == "assignment")
        .filter_map(|assignment| {
            let name = assignment.child_by_field_name("left")?.utf8_text(source).ok()?;
            Some((name.to_owned(), assignment.child_by_field_name("right")?))
        })
        .collect()
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
        let mut current = node;
        for _ in 0..MAX_ALIAS_DEPTH {
            let Some(bound) = self.binding(current) else {
                return current;
            };
            current = bound;
        }
        current
    }

    fn binding(self, node: Node<'context>) -> Option<Node<'context>> {
        if node.kind() != IDENTIFIER {
            return None;
        }
        let name = node.utf8_text(self.source).ok()?;
        self.bindings
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| *value)
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
        self.sequence(node)
            .into_iter()
            .filter_map(|element| {
                let call = Call::parse(self.resolved(element), self.source)?;
                Some(Located {
                    operation: self.operation(&call)?,
                    line: call.line,
                    column: call.column,
                })
            })
            .collect()
    }

    fn operation(self, call: &Call<'context>) -> Option<Operation> {
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
            "RunSQL" => self.run_sql(call),
            "SeparateDatabaseAndState" => Some(Operation::Separate {
                database: argument(call, 0, "database_operations")
                    .map(|node| self.operations(node))
                    .unwrap_or_default(),
                state: argument(call, 1, "state_operations")
                    .map(|node| self.operations(node))
                    .unwrap_or_default(),
            }),
            _ => self.index_operation(call),
        }
    }

    fn index_operation(self, call: &Call<'_>) -> Option<Operation> {
        let concurrently = call.method.ends_with(CONCURRENTLY);
        let model = model_name(call, 0, "model_name")?;
        match call.method.trim_end_matches(CONCURRENTLY) {
            "AddIndex" => Some(Operation::AddIndex {
                model,
                index: argument(call, 1, "index").and_then(|node| index(node, self.source))?,
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
                constraint: argument(call, 1, "constraint").and_then(|node| constraint(node, self.source))?,
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
                "indexes" => model.indexes = indexes(value, self.source),
                "constraints" => model.constraints = constraints(value, self.source),
                "managed" => model.managed = boolean(value) != Some(false),
                "proxy" => model.proxy = boolean(value) == Some(true),
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

    fn run_sql(self, call: &Call<'context>) -> Option<Operation> {
        let sql = self.resolved(argument(call, 0, "sql")?);
        let is_noop = sql.kind() == "attribute"
            && sql
                .utf8_text(self.source)
                .is_ok_and(|reference| reference.ends_with(NOOP));
        if is_noop {
            return None;
        }
        let chunks = self
            .sql_literals(sql)
            .into_iter()
            .map(|literal| self.resolved(literal))
            .filter_map(|literal| {
                Some(SqlChunk {
                    sql: text(literal, self.source)?,
                    line: one_based(literal.start_position().row),
                })
            })
            .collect();
        Some(Operation::RunSql(chunks))
    }

    fn sql_literals(self, sql: Node<'context>) -> Vec<Node<'context>> {
        if !is_sequence(sql) && sql.kind() != CONCATENATION {
            return vec![sql];
        }
        self.sequence(sql)
            .into_iter()
            .filter_map(|element| {
                let element = self.resolved(element);
                if is_sequence(element) {
                    elements(element).into_iter().next()
                } else {
                    Some(element)
                }
            })
            .collect()
    }
}
