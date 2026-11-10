use super::model::{Constraint, Field, Index, Model};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlChunk {
    pub sql: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    CreateModel(Model),
    DeleteModel {
        name: String,
        deletion: Deletion,
    },
    AddField {
        model: String,
        field: Field,
    },
    RemoveField {
        model: String,
        name: String,
        deletion: Deletion,
    },
    AlterField {
        model: String,
        field: Field,
    },
    RenameField {
        model: String,
        old: String,
        new: String,
    },
    RenameModel {
        old: String,
        new: String,
    },
    AlterModelTable {
        model: String,
        table: Option<String>,
    },
    AlterUniqueTogether {
        model: String,
        sets: Vec<Vec<String>>,
    },
    AlterManaged {
        model: String,
        managed: bool,
    },
    AddIndex {
        model: String,
        index: Index,
        concurrently: bool,
    },
    RemoveIndex {
        model: String,
        name: String,
        concurrently: bool,
    },
    RenameIndex {
        model: String,
        old: Option<String>,
        new: String,
    },
    AddConstraint {
        model: String,
        constraint: Constraint,
    },
    RemoveConstraint {
        model: String,
        name: String,
    },
    RunSql(Vec<SqlChunk>),
    CreateExtension(String),
    Separate {
        database: Vec<Located>,
        state: Vec<Located>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deletion {
    Immediate,
    MoveToPending,
    Purge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub operation: Operation,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub operations: Vec<Located>,
    pub atomic: bool,
    pub dependencies: Vec<MigrationKey>,
    pub run_before: Vec<MigrationKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MigrationKey {
    pub app: String,
    pub name: String,
}

impl MigrationKey {
    pub fn new(app: &str, name: &str) -> Self {
        Self {
            app: app.to_owned(),
            name: name.to_owned(),
        }
    }
}
