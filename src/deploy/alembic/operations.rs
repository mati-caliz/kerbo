use tree_sitter::Node;

use super::calls::{Call, string_value};
use super::render::{Constants, column, default_sql, quoted, render_column, type_sql};
use super::tables;
use crate::syntax::named_children;

const SAFE_NAME_SYMBOLS: char = '_';

pub struct Context<'context, 'tree> {
    pub value: &'context dyn Fn(Node<'_>) -> Option<String>,
    pub constants: &'context Constants<'tree>,
}

impl Context<'_, '_> {
    pub(super) fn value(&self, node: Node<'_>) -> Option<String> {
        (self.value)(node)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Target<'tree> {
    pub batch_table: Option<&'tree str>,
}

pub fn statements(call: &Call<'_>, target: Target<'_>, context: &Context<'_, '_>) -> Vec<String> {
    let offset = usize::from(target.batch_table.is_none());
    let table = |position: usize| -> String {
        target
            .batch_table
            .map(str::to_owned)
            .or_else(|| text(call, position, "table_name", context))
            .unwrap_or_default()
    };
    match call.method {
        "create_table" => tables::create_table(call, context),
        "drop_table" => vec![format!("DROP TABLE {}", name(&table(0)))],
        "rename_table" => vec![format!(
            "ALTER TABLE {} RENAME TO {}",
            name(&table(0)),
            name(&text(call, 1, "new_table_name", context).unwrap_or_default())
        )],
        "add_column" => add_column(call, &table(0), offset, context.constants),
        "drop_column" => vec![format!(
            "ALTER TABLE {} DROP COLUMN {}",
            name(&table(0)),
            name(&text(call, offset, "column_name", context).unwrap_or_default())
        )],
        "alter_column" => alter_column(call, &table(0), offset, context),
        "create_index" => vec![create_index(call, &table(1), offset, context)],
        "drop_index" => vec![format!(
            "DROP INDEX {}{}",
            if call.keyword_is_true("postgresql_concurrently") {
                "CONCURRENTLY "
            } else {
                ""
            },
            name(&text(call, 0, "index_name", context).unwrap_or_default())
        )],
        _ => constraints(call, target, offset, context),
    }
}

fn constraints(call: &Call<'_>, target: Target<'_>, offset: usize, context: &Context<'_, '_>) -> Vec<String> {
    let named = |position: usize| text(call, position, "constraint_name", context);
    let constraint_name = named(0)
        .map(|constraint| format!("CONSTRAINT {} ", name(&constraint)))
        .unwrap_or_default();
    let table_at = |position: usize, keyword: &str| {
        target
            .batch_table
            .map(str::to_owned)
            .or_else(|| text(call, position, keyword, context))
            .unwrap_or_default()
    };
    let columns_at = |position: usize, keyword: &str| columns(call, position, keyword, context);
    let not_valid = if call.keyword_is_true("postgresql_not_valid") {
        " NOT VALID"
    } else {
        ""
    };
    let shift = 1 - offset.min(1);
    let statement = match call.method {
        "create_foreign_key" => format!(
            "ALTER TABLE {} ADD {constraint_name}FOREIGN KEY({}) REFERENCES {} ({}){not_valid}",
            name(&table_at(1, "source_table")),
            columns_at(3 - shift, "local_cols"),
            name(&text(call, 2 - shift, "referent_table", context).unwrap_or_default()),
            columns_at(4 - shift, "remote_cols"),
        ),
        "create_unique_constraint" => format!(
            "ALTER TABLE {} ADD {constraint_name}UNIQUE ({})",
            name(&table_at(1, "table_name")),
            columns_at(2 - shift, "columns"),
        ),
        "create_primary_key" => format!(
            "ALTER TABLE {} ADD {constraint_name}PRIMARY KEY ({})",
            name(&table_at(1, "table_name")),
            columns_at(2 - shift, "columns"),
        ),
        "create_check_constraint" => format!(
            "ALTER TABLE {} ADD {constraint_name}CHECK ({}){not_valid}",
            name(&table_at(1, "table_name")),
            call.positional(2 - shift)
                .or_else(|| call.keyword("condition"))
                .and_then(|node| context.value(node).or_else(|| default_sql(node, call.source)))
                .unwrap_or_default(),
        ),
        "drop_constraint" => format!(
            "ALTER TABLE {} DROP CONSTRAINT {}",
            name(&table_at(1, "table_name")),
            name(&named(0).unwrap_or_default()),
        ),
        _ => return Vec::new(),
    };
    vec![statement]
}

pub(super) fn text(
    call: &Call<'_>,
    position: usize,
    keyword: &str,
    context: &Context<'_, '_>,
) -> Option<String> {
    call.positional(position)
        .or_else(|| call.keyword(keyword))
        .and_then(|node| context.value(node))
}

pub fn name(identifier: &str) -> String {
    let plain = identifier
        .starts_with(|first: char| first.is_ascii_lowercase() || first == SAFE_NAME_SYMBOLS)
        && identifier.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == SAFE_NAME_SYMBOLS
        });
    if plain {
        identifier.to_owned()
    } else {
        quoted(identifier)
    }
}

pub(super) fn columns(call: &Call<'_>, position: usize, keyword: &str, context: &Context<'_, '_>) -> String {
    call.positional(position)
        .or_else(|| call.keyword(keyword))
        .map(|list| {
            named_children(list)
                .into_iter()
                .filter_map(|item| {
                    context
                        .value(item)
                        .map(|column| name(&column))
                        .or_else(|| default_sql(item, call.source))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn add_column(call: &Call<'_>, table: &str, offset: usize, constants: &Constants<'_>) -> Vec<String> {
    let argument = call.positional(offset).map(|node| {
        node.utf8_text(call.source)
            .ok()
            .filter(|_| node.kind() == "identifier")
            .and_then(|name| constants.get(name))
            .copied()
            .unwrap_or(node)
    });
    let Some(inner) = argument.and_then(|node| Call::parse(node, call.source)) else {
        return Vec::new();
    };
    let Some(mut column) = column(&inner, constants) else {
        return Vec::new();
    };
    let references: Vec<String> = column
        .constraints
        .iter()
        .filter(|clause| clause.starts_with("REFERENCES"))
        .cloned()
        .collect();
    column
        .constraints
        .retain(|clause| !clause.starts_with("REFERENCES") && clause != "UNIQUE");
    let unique = inner.keyword_is_true("unique");
    let mut statements = vec![format!(
        "ALTER TABLE {} ADD COLUMN {}",
        name(table),
        render_column(&column)
    )];
    statements.extend(references.iter().map(|reference| {
        format!(
            "ALTER TABLE {} ADD FOREIGN KEY({}) {reference}",
            name(table),
            name(&column.name)
        )
    }));
    if column.index {
        statements.push(tables::column_index(table, &column, &inner));
    } else if unique {
        statements.push(format!(
            "ALTER TABLE {} ADD UNIQUE ({})",
            name(table),
            name(&column.name)
        ));
    }
    statements
}

fn alter_column(call: &Call<'_>, table: &str, offset: usize, context: &Context<'_, '_>) -> Vec<String> {
    let column = text(call, offset, "column_name", context).unwrap_or_default();
    let prefix = format!("ALTER TABLE {} ALTER COLUMN {}", name(table), name(&column));
    let mut statements = Vec::new();
    if let Some(sql_type) = call
        .keyword("type_")
        .and_then(|node| type_sql(node, call.source, context.constants))
    {
        statements.push(format!("{prefix} TYPE {sql_type}"));
    }
    match call.keyword("nullable").map(|value| value.kind()) {
        Some("false") => statements.push(format!("{prefix} SET NOT NULL")),
        Some("true") => statements.push(format!("{prefix} DROP NOT NULL")),
        _ => {}
    }
    if let Some(default) = call.keyword("server_default") {
        if default.kind() == "none" {
            statements.push(format!("{prefix} DROP DEFAULT"));
        } else if let Some(sql) = default_sql(default, call.source) {
            statements.push(format!("{prefix} SET DEFAULT {sql}"));
        }
    }
    if let Some(new_name) = call
        .keyword("new_column_name")
        .and_then(|node| context.value(node))
    {
        statements.push(format!(
            "ALTER TABLE {} RENAME {} TO {}",
            name(table),
            name(&column),
            name(&new_name)
        ));
    }
    statements
}

fn create_index(call: &Call<'_>, table: &str, offset: usize, context: &Context<'_, '_>) -> String {
    let index = text(call, 0, "index_name", context).unwrap_or_default();
    let unique = if call.keyword_is_true("unique") {
        "UNIQUE "
    } else {
        ""
    };
    let concurrently = if call.keyword_is_true("postgresql_concurrently") {
        "CONCURRENTLY "
    } else {
        ""
    };
    let if_not_exists = if call.keyword_is_true("if_not_exists") {
        "IF NOT EXISTS "
    } else {
        ""
    };
    let columns = columns(call, 1 + offset, "columns", context);
    format!(
        "CREATE {unique}INDEX {concurrently}{if_not_exists}{} ON {} ({columns})",
        name(&index),
        name(table)
    )
}

pub fn string_or_binding(
    node: Node<'_>,
    source: &[u8],
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let parts = named_children(node);
    if node.kind() == "string" && parts.iter().any(|part| part.kind() == "interpolation") {
        return interpolated(node, source, lookup);
    }
    if node.kind() == "concatenated_string" {
        return parts
            .into_iter()
            .map(|part| string_or_binding(part, source, lookup))
            .collect();
    }
    string_value(node, source).or_else(|| match node.kind() {
        "identifier" => node.utf8_text(source).ok().and_then(lookup),
        "call" => Call::parse(node, source)
            .filter(|inner| inner.method == "f")
            .and_then(|inner| inner.positional(0))
            .and_then(|inner| string_or_binding(inner, source, lookup)),
        _ => None,
    })
}

fn interpolated(node: Node<'_>, source: &[u8], lookup: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    named_children(node)
        .into_iter()
        .filter(|part| part.kind() == "string_content" || part.kind() == "interpolation")
        .map(|part| {
            if part.kind() == "string_content" {
                return part.utf8_text(source).ok().map(str::to_owned);
            }
            let expression = part.child_by_field_name("expression")?;
            string_or_binding(expression, source, lookup)
        })
        .collect()
}
