use tree_sitter::Node;

use super::arguments::{Arguments, names, text};
use super::columns::{Column, column, default};
use super::ddl::{ForeignKey, check_name, create_index, quote, singularize};
use super::migration::Compatibility;
use super::references::{column_index, reference};
use crate::syntax::named_children;

const ID: &str = "id";
const JOIN_SEPARATOR: char = '_';
const PRIMARY_KEY: &str = "primary_key";
const IF_NOT_EXISTS: &str = "if_not_exists";
const FORCE: &str = "force";
const CASCADE: &str = "cascade";
const PRECISION: &str = "precision";
const NAME: &str = "name";
const UUID: &str = "uuid";
const SERIAL_TYPES: &[&str] = &["serial", "integer"];
const BIGSERIAL_TYPES: &[&str] = &["bigserial", "bigint"];
const DEFAULT_PRIMARY_KEY: &str = "bigserial primary key";
const UUID_PRIMARY_KEY: &str = "uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY";
const SERIAL_PRIMARY_KEY: &str = "serial NOT NULL PRIMARY KEY";
const BIGSERIAL_PRIMARY_KEY: &str = "bigserial NOT NULL PRIMARY KEY";
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
const RECEIVER: &str = "receiver";
const METHOD: &str = "method";
const ARGUMENTS: &str = "arguments";
const CALL: &str = "call";
const GENERIC_COLUMN: &str = "column";
const DEFAULT: &str = "default";
const DATETIME: &str = "datetime";
const TABLE_NAME: &str = "table_name";
const COLUMN_OPTIONS: &str = "column_options";
const NOT_NULL: &str = "NOT NULL";
const INTEGER_ID_TYPE: &str = "integer";
const BIGINT_ID_TYPE: &str = "bigint";

struct NewTable {
    name: String,
    elements: Vec<String>,
}

pub struct TableCall<'tree> {
    pub method: String,
    pub arguments: Arguments<'tree>,
}

pub fn block_calls<'tree>(
    block: Option<Node<'tree>>,
    compatibility: Compatibility,
    source: &[u8],
) -> Vec<TableCall<'tree>> {
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
                arguments: Arguments::parse(statement.child_by_field_name(ARGUMENTS), source)
                    .with_compatibility(compatibility),
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
            default: arguments
                .keyword(DEFAULT)
                .and_then(|value| default(value, DATETIME, false, source)),
            not_null: !arguments.is_true(NULL),
            primary_key: false,
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
        Some(declared) if BIGSERIAL_TYPES.contains(&declared) => BIGSERIAL_PRIMARY_KEY,
        _ if arguments.compatibility.integer_ids => SERIAL_PRIMARY_KEY,
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
    let elements = primary_key(arguments, source).into_iter().collect();
    let new_table = NewTable {
        name: table.to_owned(),
        elements,
    };
    create_table_with(new_table, arguments, block, source)
}

pub fn create_join_table(
    first: &str,
    arguments: &Arguments<'_>,
    block: Option<Node<'_>>,
    source: &[u8],
) -> Vec<String> {
    let Some(second) = arguments.name_at(0, source) else {
        return Vec::new();
    };
    let table = arguments
        .keyword_name(TABLE_NAME, source)
        .unwrap_or_else(|| join_table_name(first, &second));
    let joined = [first.to_owned(), second];
    let column_options = arguments
        .keyword(COLUMN_OPTIONS)
        .map(|options| Arguments::parse(Some(options), source))
        .unwrap_or_default();
    let not_null = if column_options.is_true(NULL) {
        String::new()
    } else {
        format!(" {NOT_NULL}")
    };
    let id_type = if arguments.compatibility.integer_ids {
        INTEGER_ID_TYPE
    } else {
        BIGINT_ID_TYPE
    };
    let elements = joined
        .iter()
        .map(|name| {
            format!(
                "{} {id_type}{not_null}",
                quote(&format!("{}_{ID}", singularize(name)))
            )
        })
        .collect();
    create_table_with(
        NewTable {
            name: table,
            elements,
        },
        arguments,
        block,
        source,
    )
}

fn join_table_name(first: &str, second: &str) -> String {
    let (lower, upper) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let shared = lower
        .rmatch_indices(JOIN_SEPARATOR)
        .map(|(offset, _)| offset + JOIN_SEPARATOR.len_utf8())
        .find(|end| *end < lower.len() && *end < upper.len() && upper.starts_with(&lower[..*end]));
    let suffix = shared.map_or(upper, |end| &upper[end..]);
    format!("{lower}{JOIN_SEPARATOR}{suffix}")
}

fn create_table_with(
    new_table: NewTable,
    arguments: &Arguments<'_>,
    block: Option<Node<'_>>,
    source: &[u8],
) -> Vec<String> {
    let NewTable {
        name: table,
        mut elements,
    } = new_table;
    let mut statements = Vec::new();
    if arguments.keyword_name(FORCE, source).as_deref() == Some(CASCADE) {
        statements.push(format!("DROP TABLE IF EXISTS {} CASCADE", quote(&table)));
    } else if arguments.is_true(FORCE) {
        statements.push(format!("DROP TABLE IF EXISTS {}", quote(&table)));
    }
    let mut constraints = Vec::new();
    let mut indexes = Vec::new();
    for call in block_calls(block, arguments.compatibility, source) {
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
            let added = columns(method, options, source);
            elements.extend(added.iter().map(Column::render));
            indexes.extend(indexes_for(&table, &added, options, source));
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

pub fn indexes_for(table: &str, added: &[Column], options: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    added
        .iter()
        .filter_map(|column| column_index(table, std::slice::from_ref(&column.name), options, source))
        .collect()
}

pub fn reference_names(arguments: &Arguments<'_>, source: &[u8]) -> Vec<String> {
    arguments
        .positional
        .iter()
        .flat_map(|node| names(*node, source))
        .collect()
}

pub fn columns(method: &str, arguments: &Arguments<'_>, source: &[u8]) -> Vec<Column> {
    let names = if method == GENERIC_COLUMN {
        arguments.name_at(0, source).into_iter().collect()
    } else {
        reference_names(arguments, source)
    };
    names
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
