use tree_sitter::Node;

use super::arguments::Arguments;
use super::ddl::{create_index, quote};
use super::migration::Compatibility;
use super::statements::{
    add_reference, add_timestamps, change_column, remove_columns, remove_index, rename_column, statement,
};
use super::tables::{TableCall, block_calls, check_constraint, columns, indexes_for, reference_names};

const TIMESTAMPS: &str = "timestamps";
const REMOVE_TIMESTAMPS: &str = "remove_timestamps";
const REMOVE: &str = "remove";
const RENAME: &str = "rename";
const CHANGE: &str = "change";
const INDEX: &str = "index";
const REMOVE_INDEX: &str = "remove_index";
const CHECK_CONSTRAINT: &str = "check_constraint";
const REFERENCES: &[&str] = &["references", "belongs_to"];
const CHANGE_DEFAULT: &str = "change_default";
const TIMESTAMP_COLUMNS: &[&str] = &["updated_at", "created_at"];
const DELEGATES: &[(&str, &str)] = &[
    (CHANGE_DEFAULT, "change_column_default"),
    ("change_null", "change_column_null"),
    ("remove_references", "remove_reference"),
    ("remove_belongs_to", "remove_reference"),
    ("foreign_key", "add_foreign_key"),
    ("remove_foreign_key", "remove_foreign_key"),
    ("remove_check_constraint", "remove_check_constraint"),
    ("rename_index", "rename_index"),
];
const COMBINABLE: &[&str] = &[TIMESTAMPS, REMOVE_TIMESTAMPS, REMOVE, CHANGE, CHANGE_DEFAULT];
const FRAGMENT_SEPARATOR: &str = ", ";

struct Planned {
    sql: String,
    combinable: bool,
}

pub struct ChangeTable<'tree> {
    pub block: Option<Node<'tree>>,
    pub compatibility: Compatibility,
    pub bulk: bool,
}

pub fn change_table(table: &str, change: &ChangeTable<'_>, source: &[u8]) -> Vec<String> {
    let alter = alter(table);
    let planned: Vec<Planned> = block_calls(change.block, change.compatibility, source)
        .iter()
        .flat_map(|call| planned(table, &alter, call, source))
        .collect();
    if change.bulk {
        combined(&alter, planned)
    } else {
        planned.into_iter().map(|step| step.sql).collect()
    }
}

fn planned(table: &str, alter: &str, call: &TableCall<'_>, source: &[u8]) -> Vec<Planned> {
    let method = call.method.as_str();
    let options = &call.arguments;
    let statements: Vec<String> = match method {
        TIMESTAMPS => vec![add_timestamps(table, options, source)],
        REMOVE_TIMESTAMPS => vec![remove_columns(
            table,
            &TIMESTAMP_COLUMNS
                .iter()
                .map(|column| (*column).to_owned())
                .collect::<Vec<_>>(),
        )],
        REMOVE => vec![remove_columns(table, &reference_names(options, source))],
        RENAME => rename_column(table, options, source).into_iter().collect(),
        CHANGE => change_column(table, options, source).into_iter().collect(),
        REMOVE_INDEX => vec![remove_index(table, options, source)],
        INDEX => options
            .positional
            .first()
            .map(|columns| create_index(table, *columns, options, source))
            .into_iter()
            .collect(),
        CHECK_CONSTRAINT => check_constraint(table, options, source)
            .map(|constraint| format!("{alter} ADD {constraint}"))
            .into_iter()
            .collect(),
        _ if REFERENCES.contains(&method) => reference_names(options, source)
            .iter()
            .flat_map(|name| add_reference(table, name, options, source))
            .collect(),
        _ => {
            if let Some((_, delegate)) = DELEGATES.iter().find(|(name, _)| *name == method) {
                statement(delegate, table, options, source).unwrap_or_default()
            } else {
                return added_columns(table, method, options, source);
            }
        }
    };
    let combinable = COMBINABLE.contains(&method);
    statements
        .into_iter()
        .map(|sql| Planned { sql, combinable })
        .collect()
}

fn alter(table: &str) -> String {
    format!("ALTER TABLE {}", quote(table))
}

fn added_columns(table: &str, method: &str, options: &Arguments<'_>, source: &[u8]) -> Vec<Planned> {
    let alter = alter(table);
    let added = columns(method, options, source);
    let additions = added.iter().map(|column| Planned {
        sql: format!("{alter} ADD {}", column.render()),
        combinable: true,
    });
    let indexes = indexes_for(table, &added, options, source)
        .into_iter()
        .map(|sql| Planned {
            sql,
            combinable: false,
        });
    additions.chain(indexes).collect()
}

fn combined(alter: &str, planned: Vec<Planned>) -> Vec<String> {
    let prefix = format!("{alter} ");
    let mut statements = Vec::new();
    let mut fragments: Vec<String> = Vec::new();
    for step in planned {
        if step.combinable
            && let Some(fragment) = step.sql.strip_prefix(&prefix)
        {
            fragments.push(fragment.to_owned());
            continue;
        }
        flush(alter, &mut fragments, &mut statements);
        statements.push(step.sql);
    }
    flush(alter, &mut fragments, &mut statements);
    statements
}

fn flush(alter: &str, fragments: &mut Vec<String>, statements: &mut Vec<String>) {
    if !fragments.is_empty() {
        statements.push(format!("{alter} {}", fragments.join(FRAGMENT_SEPARATOR)));
        fragments.clear();
    }
}
