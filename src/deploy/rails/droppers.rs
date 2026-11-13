use tree_sitter::Node;

use super::arguments::{name, names};
use crate::syntax::named_children;

const ROOT_PREFIX: &str = "::";
const NAMESPACE_PREFIX: &str = "Migration::";
const COLUMN_DROPPER: &str = "ColumnDropper";
const TABLE_DROPPER: &str = "TableDropper";
const EXECUTE_DROP: &str = "execute_drop";
const DROP_READONLY: &str = "drop_readonly";
const MARK_READONLY: &str = "mark_readonly";
const READ_ONLY_TABLE: &str = "read_only_table";
const FUNCTION_SCHEMA: &str = "discourse_functions";

pub fn statements(
    receiver: &str,
    method: &str,
    arguments: Option<Node<'_>>,
    source: &[u8],
) -> Option<Vec<String>> {
    let dropper = receiver
        .trim_start_matches(ROOT_PREFIX)
        .trim_start_matches(NAMESPACE_PREFIX);
    let positional = arguments.map(named_children).unwrap_or_default();
    let table = name(*positional.first()?, source)?;
    let columns = positional.get(1).map(|node| names(*node, source));
    match (dropper, method, columns) {
        (COLUMN_DROPPER, EXECUTE_DROP, Some(columns)) => Some(
            columns
                .iter()
                .flat_map(|column| {
                    let mut statements = drop_readonly(&format!("{table}_{column}"));
                    statements.push(format!("ALTER TABLE {table} DROP COLUMN IF EXISTS {column}"));
                    statements
                })
                .collect(),
        ),
        (COLUMN_DROPPER, DROP_READONLY, Some(columns)) => Some(
            columns
                .iter()
                .flat_map(|column| drop_readonly(&format!("{table}_{column}")))
                .collect(),
        ),
        (COLUMN_DROPPER, MARK_READONLY, Some(columns)) => Some(
            columns
                .iter()
                .flat_map(|column| mark_readonly_column(&table, column))
                .collect(),
        ),
        (TABLE_DROPPER, EXECUTE_DROP, None) => Some(vec![
            format!("DROP TABLE IF EXISTS {table}"),
            drop_function(&qualified_function(&table)),
        ]),
        (TABLE_DROPPER, READ_ONLY_TABLE, None) => Some(mark_readonly_table(&table)),
        _ => None,
    }
}

fn function_name(target: &str) -> String {
    format!("raise_{target}_readonly()")
}

fn qualified_function(target: &str) -> String {
    format!("{FUNCTION_SCHEMA}.{}", function_name(target))
}

fn drop_function(function: &str) -> String {
    format!("DROP FUNCTION IF EXISTS {function} CASCADE")
}

fn drop_readonly(target: &str) -> Vec<String> {
    vec![
        drop_function(&qualified_function(target)),
        drop_function(&function_name(target)),
    ]
}

fn readonly_function(target: &str, message: &str) -> Vec<String> {
    vec![
        format!("CREATE SCHEMA IF NOT EXISTS {FUNCTION_SCHEMA}"),
        format!(
            "CREATE OR REPLACE FUNCTION {} RETURNS trigger AS $rcr$\n  BEGIN\n    RAISE EXCEPTION 'Discourse: {message}';\n  END\n$rcr$ LANGUAGE plpgsql",
            qualified_function(target)
        ),
    ]
}

fn mark_readonly_column(table: &str, column: &str) -> Vec<String> {
    let target = format!("{table}_{column}");
    let mut created = readonly_function(&target, &format!("{column} in {table} is readonly"));
    created.push(format!(
        "CREATE TRIGGER {target}_readonly\nBEFORE INSERT OR UPDATE OF {column}\nON {table}\nFOR EACH ROW\nWHEN (NEW.{column} IS NOT NULL)\nEXECUTE PROCEDURE {}",
        qualified_function(&target)
    ));
    created
}

fn mark_readonly_table(table: &str) -> Vec<String> {
    let mut created = readonly_function(table, &format!("{table} is read only"));
    created.push(format!(
        "CREATE TRIGGER {table}_readonly\nBEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE\nON {table}\nFOR EACH STATEMENT\nEXECUTE PROCEDURE {}",
        qualified_function(table)
    ));
    created
}
