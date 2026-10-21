use super::calls::{Call, string_value};
use super::operations::{Context, columns, name, text};
use super::render::{Column, column, make_serial, render_column};

const ENUM_TYPES: &[&str] = &["Enum", "ENUM"];
const PRIMARY_KEY_CONSTRAINT: &str = "PrimaryKeyConstraint";

pub(super) fn create_table(call: &Call<'_>, context: &Context<'_, '_>) -> Vec<String> {
    let table = text(call, 0, "table_name", context).unwrap_or_default();
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut columns = Vec::new();
    let mut constraints = Vec::new();
    let mut key_columns: Vec<String> = Vec::new();
    for argument in call.all_positional().into_iter().skip(1) {
        let Some(inner) = Call::parse(argument, call.source) else {
            continue;
        };
        if inner.method == "Column" {
            let Some(column) = column(&inner, context.constants) else {
                continue;
            };
            before.extend(created_enum(&inner, context));
            if column.index {
                after.push(column_index(&table, &column, &inner));
            }
            if column.primary_key {
                key_columns.push(column.name.clone());
            }
            columns.push(column);
        } else {
            if inner.method == PRIMARY_KEY_CONSTRAINT {
                key_columns.extend(
                    inner
                        .all_positional()
                        .into_iter()
                        .filter_map(|node| context.value(node)),
                );
            }
            constraints.extend(table_constraint(&inner, context));
        }
    }
    let single_key = key_columns.len() == 1;
    let mut definitions: Vec<String> = columns
        .iter_mut()
        .map(|column| {
            if key_columns.contains(&column.name) {
                make_serial(column, single_key);
            }
            render_column(column)
        })
        .collect();
    definitions.extend(constraints);
    before.push(format!(
        "CREATE TABLE {} ({})",
        name(&table),
        definitions.join(", ")
    ));
    before.extend(after);
    before
}

pub(super) fn created_enum(column: &Call<'_>, context: &Context<'_, '_>) -> Option<String> {
    let enumeration = column
        .all_positional()
        .into_iter()
        .filter_map(|node| Call::parse(node, column.source))
        .find(|inner| ENUM_TYPES.contains(&inner.method))?;
    let type_name = enumeration
        .keyword("name")
        .and_then(|node| string_value(node, column.source))?;
    let labels: Vec<String> = enumeration
        .all_positional()
        .into_iter()
        .filter_map(|node| context.value(node))
        .map(|label| format!("'{}'", label.replace('\'', "''")))
        .collect();
    Some(format!(
        "CREATE TYPE {} AS ENUM ({})",
        name(&type_name),
        labels.join(", ")
    ))
}

pub(super) fn table_constraint(inner: &Call<'_>, context: &Context<'_, '_>) -> Option<String> {
    let listed = |constraint: &Call<'_>| {
        constraint
            .all_positional()
            .into_iter()
            .filter_map(|node| context.value(node))
            .map(|column| name(&column))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match inner.method {
        "PrimaryKeyConstraint" => Some(format!("PRIMARY KEY ({})", listed(inner))),
        "UniqueConstraint" => Some(format!("UNIQUE ({})", listed(inner))),
        "ForeignKeyConstraint" => Some(format!(
            "FOREIGN KEY({}) REFERENCES {}",
            columns(inner, 0, "columns", context),
            columns(inner, 1, "refcolumns", context)
        )),
        "CheckConstraint" => inner
            .positional(0)
            .and_then(|node| context.value(node))
            .map(|condition| format!("CHECK ({condition})")),
        _ => None,
    }
}

pub(super) fn column_index(table: &str, column: &Column, inner: &Call<'_>) -> String {
    let unique = if inner.keyword_is_true("unique") {
        "UNIQUE "
    } else {
        ""
    };
    format!(
        "CREATE {unique}INDEX {} ON {} ({})",
        name(&format!("ix_{table}_{}", column.name)),
        name(table),
        name(&column.name)
    )
}
