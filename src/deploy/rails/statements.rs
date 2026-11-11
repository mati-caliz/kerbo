use super::arguments::{Arguments, is_nil, names, sql_literal, text};
use super::columns::{Column, column, default, sql_type};
use super::ddl::{ForeignKey, check_name, create_index, drop_index, quote};
use super::references::reference;
use super::tables::timestamp_columns;

const IF_NOT_EXISTS: &str = "if_not_exists";
const IF_EXISTS: &str = "if_exists";
const FORCE: &str = "force";
const CASCADE: &str = "cascade";
const NAME: &str = "name";
const USING: &str = "using";
const DEFAULT: &str = "default";
const NULL: &str = "null";
const TO: &str = "to";
const TO_TABLE: &str = "to_table";
const VALIDATE: &str = "validate";
const FOREIGN_KEY: &str = "foreign_key";
const CREATED_AT: &str = "created_at";
const UPDATED_AT: &str = "updated_at";

fn alter(table: &str) -> String {
    format!("ALTER TABLE {}", quote(table))
}

fn first_name(arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    arguments.name_at(0, source)
}

pub fn statement(method: &str, table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<Vec<String>> {
    let statements = match method {
        "add_column" => add_column(table, arguments, source).into_iter().collect(),
        "remove_column" => remove_column(table, arguments, source).into_iter().collect(),
        "remove_columns" => vec![remove_columns(table, &positional_names(arguments, source))],
        "rename_column" => rename_column(table, arguments, source).into_iter().collect(),
        "rename_table" => rename_table(table, arguments, source),
        "add_index" => add_index(table, arguments, source).into_iter().collect(),
        "remove_index" => vec![remove_index(table, arguments, source)],
        "rename_index" => rename_index(arguments, source).into_iter().collect(),
        "add_reference" | "add_belongs_to" => references(table, arguments, source, add_reference),
        "remove_reference" | "remove_belongs_to" => references(table, arguments, source, remove_reference),
        "add_foreign_key" => add_foreign_key(table, arguments, source).into_iter().collect(),
        "remove_foreign_key" => vec![format!(
            "{} DROP CONSTRAINT {}",
            alter(table),
            quote(&foreign_key(arguments, source).constraint_name(table))
        )],
        "validate_foreign_key" => vec![format!(
            "{} VALIDATE CONSTRAINT {}",
            alter(table),
            quote(&foreign_key(arguments, source).constraint_name(table))
        )],
        "change_column" => change_column(table, arguments, source).into_iter().collect(),
        "change_column_null" => change_column_null(table, arguments, source),
        "change_column_default" => change_column_default(table, arguments, source)
            .into_iter()
            .collect(),
        "add_check_constraint" => add_check_constraint(table, arguments, source)
            .into_iter()
            .collect(),
        "remove_check_constraint" => check_constraint_name(table, arguments, source)
            .map(|name| format!("{} DROP CONSTRAINT {name}", alter(table)))
            .into_iter()
            .collect(),
        "validate_check_constraint" => check_constraint_name(table, arguments, source)
            .map(|name| format!("{} VALIDATE CONSTRAINT {name}", alter(table)))
            .into_iter()
            .collect(),
        "add_timestamps" => vec![add_timestamps(table, arguments, source)],
        "remove_timestamps" => vec![remove_columns(
            table,
            &[CREATED_AT.to_owned(), UPDATED_AT.to_owned()],
        )],
        "drop_table" => vec![drop_table(table, arguments, source)],
        "enable_extension" => vec![format!("CREATE EXTENSION IF NOT EXISTS {}", quote(table))],
        "disable_extension" => vec![format!("DROP EXTENSION IF EXISTS {} CASCADE", quote(table))],
        "create_enum" => vec![create_enum(table, arguments, source)],
        "drop_enum" => vec![format!("DROP TYPE {}{}", if_exists(arguments), quote(table))],
        _ => return None,
    };
    Some(statements)
}

fn if_exists(arguments: &Arguments<'_>) -> &'static str {
    if arguments.is_true(IF_EXISTS) {
        "IF EXISTS "
    } else {
        ""
    }
}

fn positional_names(arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    arguments
        .positional
        .iter()
        .flat_map(|node| names(*node, source))
        .collect()
}

fn add_column(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let name = first_name(arguments, source)?;
    let kind = arguments.name_at(1, source)?;
    let added = column(&kind, name, arguments, source);
    let if_not_exists = if arguments.is_true(IF_NOT_EXISTS) {
        "COLUMN IF NOT EXISTS "
    } else {
        ""
    };
    Some(format!("{} ADD {if_not_exists}{}", alter(table), added.render()))
}

fn remove_column(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let name = first_name(arguments, source)?;
    Some(format!(
        "{} DROP COLUMN {}{}",
        alter(table),
        if_exists(arguments),
        quote(&name)
    ))
}

pub fn remove_columns(table: &str, columns: &[String]) -> String {
    let drops = columns
        .iter()
        .map(|column| format!("DROP COLUMN {}", quote(column)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} {drops}", alter(table))
}

pub fn rename_column(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let from = first_name(arguments, source)?;
    let to = arguments.name_at(1, source)?;
    Some(format!(
        "{} RENAME COLUMN {} TO {}",
        alter(table),
        quote(&from),
        quote(&to)
    ))
}

fn rename_table(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    let Some(to) = first_name(arguments, source) else {
        return Vec::new();
    };
    vec![format!("{} RENAME TO {}", alter(table), quote(&to))]
}

fn add_index(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let columns = arguments.positional.first()?;
    Some(create_index(table, *columns, arguments, source))
}

pub fn remove_index(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    drop_index(table, arguments, source)
}

fn rename_index(arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let from = first_name(arguments, source)?;
    let to = arguments.name_at(1, source)?;
    Some(format!("ALTER INDEX {} RENAME TO {}", quote(&from), quote(&to)))
}

fn references(
    table: &str,
    arguments: &Arguments<'_>,
    source: &[u8],
    build: fn(&str, &str, &Arguments<'_>, &[u8]) -> Vec<String>,
) -> Vec<String> {
    positional_names(arguments, source)
        .iter()
        .flat_map(|name| build(table, name, arguments, source))
        .collect()
}

pub fn add_reference(table: &str, name: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    let built = reference(table, name, arguments, source);
    let mut statements: Vec<String> = built
        .columns
        .iter()
        .map(|column| format!("{} ADD {}", alter(table), column.render()))
        .collect();
    statements.extend(built.index);
    statements.extend(built.foreign_key.map(|key| key.add(table)));
    statements
}

fn remove_reference(table: &str, name: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    let built = reference(table, name, arguments, source);
    let mut statements: Vec<String> = built
        .foreign_key
        .filter(|_| arguments.has(FOREIGN_KEY))
        .map(|key| {
            format!(
                "{} DROP CONSTRAINT {}",
                alter(table),
                quote(&key.constraint_name(table))
            )
        })
        .into_iter()
        .collect();
    let columns: Vec<String> = built
        .columns
        .into_iter()
        .map(|column| column.name)
        .rev()
        .collect();
    if !columns.is_empty() {
        statements.push(remove_columns(table, &columns));
    }
    statements
}

fn foreign_key(arguments: &Arguments<'_>, source: &[u8]) -> ForeignKey {
    let to_table = first_name(arguments, source)
        .or_else(|| arguments.keyword_name(TO_TABLE, source))
        .unwrap_or_default();
    ForeignKey::new(to_table, arguments, source)
}

fn add_foreign_key(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    first_name(arguments, source)?;
    Some(foreign_key(arguments, source).add(table))
}

pub fn change_column(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let name = first_name(arguments, source)?;
    let declared = arguments.name_at(1, source)?;
    let target = quote(&name);
    let mut changes = vec![format!(
        "ALTER COLUMN {target} TYPE {}{}",
        sql_type(&declared, arguments, source),
        arguments
            .keyword(USING)
            .map(|expression| format!(" USING {}", text(expression, source)))
            .unwrap_or_default()
    )];
    if let Some(value) = arguments.keyword(DEFAULT) {
        changes.push(default(value, &declared, false, source).map_or_else(
            || format!("ALTER COLUMN {target} DROP DEFAULT"),
            |value| format!("ALTER COLUMN {target} SET DEFAULT {value}"),
        ));
    }
    if arguments.is_false(NULL) {
        changes.push(format!("ALTER COLUMN {target} SET NOT NULL"));
    } else if arguments.is_true(NULL) {
        changes.push(format!("ALTER COLUMN {target} DROP NOT NULL"));
    }
    Some(format!("{} {}", alter(table), changes.join(", ")))
}

fn change_column_null(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    let Some(name) = first_name(arguments, source) else {
        return Vec::new();
    };
    let target = quote(&name);
    let allows_null = arguments
        .positional
        .get(1)
        .is_none_or(|value| value.kind() != "false");
    let mut statements: Vec<String> = arguments
        .positional
        .get(2)
        .filter(|value| !is_nil(**value))
        .and_then(|value| sql_literal(*value, source))
        .map(|value| {
            format!(
                "UPDATE {} SET {target}={value} WHERE {target} IS NULL",
                quote(table)
            )
        })
        .into_iter()
        .collect();
    let change = if allows_null {
        "DROP NOT NULL"
    } else {
        "SET NOT NULL"
    };
    statements.push(format!("{} ALTER COLUMN {target} {change}", alter(table)));
    statements
}

fn change_column_default(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let name = first_name(arguments, source)?;
    let value = arguments
        .keyword(TO)
        .or_else(|| arguments.positional.get(1).copied())?;
    let target = quote(&name);
    Some(default(value, "", false, source).map_or_else(
        || format!("{} ALTER COLUMN {target} DROP DEFAULT", alter(table)),
        |value| format!("{} ALTER COLUMN {target} SET DEFAULT {value}", alter(table)),
    ))
}

fn check_constraint_name(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    arguments.keyword_name(NAME, source).or_else(|| {
        let expression = text(*arguments.positional.first()?, source);
        Some(check_name(table, &expression))
    })
}

fn add_check_constraint(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let expression = text(*arguments.positional.first()?, source);
    let name = check_constraint_name(table, arguments, source)?;
    let not_valid = if arguments.is_false(VALIDATE) {
        " NOT VALID"
    } else {
        ""
    };
    Some(format!(
        "{} ADD CONSTRAINT {name} CHECK ({expression}){not_valid}",
        alter(table)
    ))
}

pub fn add_timestamps(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    let additions = timestamp_columns(arguments, source)
        .iter()
        .map(|column: &Column| format!("ADD {}", column.render()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} {additions}", alter(table))
}

fn drop_table(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    let cascade = if arguments.keyword_name(FORCE, source).as_deref() == Some(CASCADE) {
        " CASCADE"
    } else {
        ""
    };
    format!("DROP TABLE {}{}{cascade}", if_exists(arguments), quote(table))
}

fn create_enum(name: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    let values = positional_names(arguments, source)
        .iter()
        .map(|value| super::arguments::quoted(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("CREATE TYPE {} AS ENUM ({values})", quote(name))
}
