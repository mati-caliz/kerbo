use tree_sitter::Node;

use super::arguments::{
    Arguments, is_empty_collection, is_hash, is_nil, lambda_body, quoted, raw, sql_literal, text,
};

const VARCHAR: &str = "character varying";
const TIMESTAMP: &str = "timestamp";
const BIGINT: &str = "bigint";
const SMALLINT: &str = "smallint";
const INTEGER: &str = "integer";
const DECIMAL: &str = "decimal";
const SMALLINT_LIMIT: u8 = 2;
const INTEGER_LIMIT: u8 = 4;
const ARRAY_SUFFIX: &str = "[]";
const EMPTY_ARRAY: &str = "'{}'";
const EMPTY_JSON_ARRAY: &str = "'[]'";
const EMPTY_JSON_OBJECT: &str = "'{}'";
const JSON_TYPES: &[&str] = &["json", "jsonb"];
const RENAMED_TYPES: &[(&str, &str)] = &[
    ("string", VARCHAR),
    ("datetime", TIMESTAMP),
    ("binary", "bytea"),
    (PRIMARY_KEY, "bigserial primary key"),
    ("timestamptz", "timestamp with time zone"),
];
const VIRTUAL: &str = "virtual";
const VECTOR: &str = "vector";
const GENERIC_COLUMN: &str = "column";
const TYPE: &str = "type";
const LIMIT: &str = "limit";
const PRECISION: &str = "precision";
const SCALE: &str = "scale";
const ARRAY: &str = "array";
const DEFAULT: &str = "default";
const NULL: &str = "null";
const AS: &str = "as";
const STORED: &str = "stored";
const ENUM: &str = "enum";
const ENUM_TYPE: &str = "enum_type";
const PRIMARY_KEY: &str = "primary_key";
const INTEGER_PRIMARY_KEY: &str = "serial NOT NULL PRIMARY KEY";
const PRIMARY_KEY_SUFFIX: &str = " PRIMARY KEY";
const SERIAL_TYPES: &[(&str, &str)] = &[(INTEGER, "serial"), (BIGINT, "bigserial")];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub sql_type: String,
    pub default: Option<String>,
    pub not_null: bool,
    pub primary_key: bool,
}

impl Column {
    pub fn render(&self) -> String {
        let default = self
            .default
            .as_ref()
            .map(|default| format!(" DEFAULT {default}"))
            .unwrap_or_default();
        let not_null = if self.not_null { " NOT NULL" } else { "" };
        let primary_key = if self.primary_key { PRIMARY_KEY_SUFFIX } else { "" };
        format!(
            "\"{}\" {}{default}{not_null}{primary_key}",
            self.name, self.sql_type
        )
    }
}

pub fn column(kind: &str, name: String, arguments: &Arguments<'_>, source: &[u8]) -> Column {
    let declared = if kind == GENERIC_COLUMN {
        arguments.name_at(1, source).unwrap_or_default()
    } else if kind == VIRTUAL {
        arguments.keyword_name(TYPE, source).unwrap_or_default()
    } else {
        kind.to_owned()
    };
    let mut sql_type = sql_type(&declared, arguments, source);
    let is_array = arguments.is_true(ARRAY);
    if is_array {
        sql_type.push_str(ARRAY_SUFFIX);
    }
    let primary_key = arguments.is_true(PRIMARY_KEY);
    if primary_key {
        sql_type = serial(sql_type);
    }
    if kind == VIRTUAL {
        let expression = arguments
            .keyword(AS)
            .map(|node| text(node, source))
            .unwrap_or_default();
        let stored = if arguments.is_true(STORED) { " STORED" } else { "" };
        sql_type = format!("{sql_type} GENERATED ALWAYS AS ({expression}){stored}");
    }
    Column {
        name,
        default: arguments
            .keyword(DEFAULT)
            .and_then(|value| default(value, &declared, is_array, source)),
        not_null: primary_key || arguments.is_false(NULL),
        sql_type,
        primary_key,
    }
}

fn serial(sql_type: String) -> String {
    SERIAL_TYPES
        .iter()
        .find_map(|(integer, serial)| (*integer == sql_type).then(|| (*serial).to_owned()))
        .unwrap_or(sql_type)
}

pub fn sql_type(declared: &str, arguments: &Arguments<'_>, source: &[u8]) -> String {
    let limit = arguments.number(LIMIT, source);
    if declared == INTEGER {
        return match limit.as_deref().and_then(|limit| limit.parse::<u8>().ok()) {
            Some(bytes) if bytes <= SMALLINT_LIMIT => SMALLINT,
            Some(bytes) if bytes > INTEGER_LIMIT => BIGINT,
            _ => INTEGER,
        }
        .to_owned();
    }
    if declared == ENUM {
        return arguments
            .keyword_name(ENUM_TYPE, source)
            .map_or_else(|| ENUM.to_owned(), |name| format!("\"{name}\""));
    }
    if declared == PRIMARY_KEY && arguments.compatibility.integer_ids {
        return INTEGER_PRIMARY_KEY.to_owned();
    }
    if declared == DECIMAL {
        return decimal(arguments, source);
    }
    let base = RENAMED_TYPES
        .iter()
        .find_map(|(rails, sql)| (*rails == declared).then_some(*sql))
        .unwrap_or(declared);
    let size = if base == TIMESTAMP {
        arguments.number(PRECISION, source)
    } else {
        limit.filter(|_| base == VARCHAR || declared == VECTOR)
    };
    size.map_or_else(|| base.to_owned(), |size| format!("{base}({size})"))
}

fn decimal(arguments: &Arguments<'_>, source: &[u8]) -> String {
    match (
        arguments.number(PRECISION, source),
        arguments.number(SCALE, source),
    ) {
        (Some(precision), Some(scale)) => format!("{DECIMAL}({precision},{scale})"),
        (Some(precision), None) => format!("{DECIMAL}({precision})"),
        (None, _) => DECIMAL.to_owned(),
    }
}

pub fn default(value: Node<'_>, declared: &str, is_array: bool, source: &[u8]) -> Option<String> {
    if is_nil(value) {
        return None;
    }
    if let Some(expression) = lambda_body(value, source) {
        return Some(expression);
    }
    if let Some(empty) = is_empty_collection(value) {
        if is_array {
            return Some(EMPTY_ARRAY.to_owned());
        }
        if JSON_TYPES.contains(&declared) && empty {
            return Some(
                if is_hash(value) {
                    EMPTY_JSON_OBJECT
                } else {
                    EMPTY_JSON_ARRAY
                }
                .to_owned(),
            );
        }
        return Some(quoted(&raw(value, source)));
    }
    sql_literal(value, source)
}
