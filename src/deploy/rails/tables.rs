use tree_sitter::Node;

use super::arguments::{Arguments, names, text};
use super::columns::{Column, column};
use super::ddl::{ForeignKey, check_name, create_index, quote};
use super::references::reference;
use super::statements::{
    add_reference, add_timestamps, change_column, remove_columns, remove_index, rename_column,
};
use crate::syntax::named_children;

const ID: &str = "id";
const PRIMARY_KEY: &str = "primary_key";
const IF_NOT_EXISTS: &str = "if_not_exists";
const FORCE: &str = "force";
const CASCADE: &str = "cascade";
const PRECISION: &str = "precision";
const NAME: &str = "name";
const UUID: &str = "uuid";
const SERIAL_TYPES: &[&str] = &["serial", "integer"];
const DEFAULT_PRIMARY_KEY: &str = "bigserial primary key";
const UUID_PRIMARY_KEY: &str = "uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY";
const SERIAL_PRIMARY_KEY: &str = "serial NOT NULL PRIMARY KEY";
const DEFAULT_PRECISION: &str = "6";
const CREATED_AT: &str = "created_at";
const UPDATED_AT: &str = "updated_at";
const TIMESTAMP: &str = "timestamp";
const NULL: &str = "null";
const TIMESTAMPS: &str = "timestamps";
const REFERENCES: &[&str] = &["references", "belongs_to"];
const INDEX: &str = "index";
const FOREIGN_KEY: &str = "foreign_key";
const CHECK_CONSTRAINT: &str = "check_constraint";
const REMOVE: &str = "remove";
const RENAME: &str = "rename";
const CHANGE: &str = "change";
const REMOVE_INDEX: &str = "remove_index";
const REMOVE_TIMESTAMPS: &str = "remove_timestamps";
const RECEIVER: &str = "receiver";
const METHOD: &str = "method";
const ARGUMENTS: &str = "arguments";
const CALL: &str = "call";

pub struct TableCall<'tree> {
    pub method: String,
    pub arguments: Arguments<'tree>,
}

pub fn block_calls<'tree>(block: Option<Node<'tree>>, source: &[u8]) -> Vec<TableCall<'tree>> {
    let Some(block) = block else {
        return Vec::new();
    };
    let body = block.child_by_field_name("body");
    body.map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|statement| statement.kind() == CALL && statement.child_by_field_name(RECEIVER).is_some())
        .filter_map(|statement| {
            Some(TableCall {
                method: super::arguments::raw(statement.child_by_field_name(METHOD)?, source),
                arguments: Arguments::parse(statement.child_by_field_name(ARGUMENTS), source),
            })
        })
        .collect()
}

pub fn timestamp_columns(arguments: &Arguments<'_>, source: &[u8]) -> Vec<Column> {
    let precision = arguments
        .number(PRECISION, source)
        .unwrap_or_else(|| DEFAULT_PRECISION.to_owned());
    [CREATED_AT, UPDATED_AT]
        .into_iter()
        .map(|name| Column {
            name: name.to_owned(),
            sql_type: format!("{TIMESTAMP}({precision})"),
            default: None,
            not_null: !arguments.is_true(NULL),
        })
        .collect()
}

fn primary_key(arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    if arguments.is_false(ID) {
        return None;
    }
    let name = arguments
        .keyword_name(PRIMARY_KEY, source)
        .unwrap_or_else(|| ID.to_owned());
    let kind = arguments.keyword_name(ID, source);
    let definition = match kind.as_deref() {
        Some(UUID) => UUID_PRIMARY_KEY,
        Some(declared) if SERIAL_TYPES.contains(&declared) => SERIAL_PRIMARY_KEY,
        _ => DEFAULT_PRIMARY_KEY,
    };
    Some(format!("{} {definition}", quote(&name)))
}

pub fn create_table(
    table: &str,
    arguments: &Arguments<'_>,
    block: Option<Node<'_>>,
    source: &[u8],
) -> Vec<String> {
    let table = table.to_owned();
    let mut statements = Vec::new();
    if arguments.keyword_name(FORCE, source).as_deref() == Some(CASCADE) {
        statements.push(format!("DROP TABLE IF EXISTS {} CASCADE", quote(&table)));
    }
    let mut elements: Vec<String> = primary_key(arguments, source).into_iter().collect();
    let mut constraints = Vec::new();
    let mut indexes = Vec::new();
    for call in block_calls(block, source) {
        let method = call.method.as_str();
        let options = &call.arguments;
        if method == TIMESTAMPS {
            elements.extend(timestamp_columns(options, source).iter().map(Column::render));
        } else if REFERENCES.contains(&method) {
            for name in reference_names(options, source) {
                let built = reference(&table, &name, options, source);
                elements.extend(built.columns.iter().map(Column::render));
                indexes.extend(built.index);
                constraints.extend(built.foreign_key.map(|key| key.constraint(&table)));
            }
        } else if method == INDEX {
            indexes.extend(
                options
                    .positional
                    .first()
                    .map(|columns| create_index(&table, *columns, options, source)),
            );
        } else if method == FOREIGN_KEY {
            constraints.extend(
                options
                    .name_at(0, source)
                    .map(|target| ForeignKey::new(target, options, source).constraint(&table)),
            );
        } else if method == CHECK_CONSTRAINT {
            constraints.extend(check_constraint(&table, options, source));
        } else {
            elements.extend(columns(method, options, source).iter().map(Column::render));
        }
    }
    elements.extend(constraints);
    let if_not_exists = if arguments.is_true(IF_NOT_EXISTS) {
        "IF NOT EXISTS "
    } else {
        ""
    };
    statements.push(format!(
        "CREATE TABLE {if_not_exists}{} ({})",
        quote(&table),
        elements.join(", ")
    ));
    statements.extend(indexes);
    statements
}

fn reference_names(arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    arguments
        .positional
        .iter()
        .flat_map(|node| names(*node, source))
        .collect()
}

pub fn columns(method: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<Column> {
    reference_names(arguments, source)
        .into_iter()
        .map(|name| column(method, name, arguments, source))
        .collect()
}

pub fn check_constraint(table: &str, arguments: &Arguments<'_>, source: &[u8]) -> Option<String> {
    let expression = text(*arguments.positional.first()?, source);
    let name = arguments
        .keyword_name(NAME, source)
        .unwrap_or_else(|| check_name(table, &expression));
    Some(format!("CONSTRAINT {name} CHECK ({expression})"))
}

pub fn change_table(table: &str, block: Option<Node<'_>>, source: &[u8]) -> Vec<String> {
    let table = table.to_owned();
    let alter = format!("ALTER TABLE {}", quote(&table));
    let mut statements = Vec::new();
    for call in block_calls(block, source) {
        let method = call.method.as_str();
        let options = &call.arguments;
        match method {
            TIMESTAMPS => statements.push(add_timestamps(&table, options, source)),
            REMOVE_TIMESTAMPS => statements.push(remove_columns(
                &table,
                &[CREATED_AT.to_owned(), UPDATED_AT.to_owned()],
            )),
            REMOVE => statements.push(remove_columns(&table, &reference_names(options, source))),
            RENAME => statements.extend(rename_column(&table, options, source)),
            CHANGE => statements.extend(change_column(&table, options, source)),
            REMOVE_INDEX => statements.push(remove_index(&table, options, source)),
            INDEX => statements.extend(
                options
                    .positional
                    .first()
                    .map(|columns| create_index(&table, *columns, options, source)),
            ),
            CHECK_CONSTRAINT => statements.extend(
                check_constraint(&table, options, source)
                    .map(|constraint| format!("{alter} ADD {constraint}")),
            ),
            _ if REFERENCES.contains(&method) => {
                for name in reference_names(options, source) {
                    statements.extend(add_reference(&table, &name, options, source));
                }
            }
            _ => statements.extend(
                columns(method, options, source)
                    .iter()
                    .map(|column| format!("{alter} ADD {}", column.render())),
            ),
        }
    }
    statements
}
