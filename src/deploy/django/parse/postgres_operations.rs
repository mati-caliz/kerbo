use super::super::dispatch::TABLE_PLACEHOLDER;
use super::super::names::{quoted, sql_string};
use super::super::operations::Operation;
use super::super::values::{boolean, text};
use super::{model_name, string_argument};
use crate::deploy::python_calls::Call;

const CREATE_COLLATION: &str = "CreateCollation";
const REMOVE_COLLATION: &str = "RemoveCollation";
const VALIDATE_CONSTRAINT: &str = "ValidateConstraint";
const ALTER_TABLE_COMMENT: &str = "AlterModelTableComment";
const GENERIC_EXTENSION: &str = "CreateExtension";
const NAMED_EXTENSIONS: &[(&str, &str)] = &[
    ("BloomExtension", "bloom"),
    ("BtreeGinExtension", "btree_gin"),
    ("BtreeGistExtension", "btree_gist"),
    ("CITextExtension", "citext"),
    ("CryptoExtension", "pgcrypto"),
    ("HStoreExtension", "hstore"),
    ("TrigramExtension", "pg_trgm"),
    ("UnaccentExtension", "unaccent"),
];
const DEFAULT_PROVIDER: &str = "libc";
const NON_DETERMINISTIC: &str = "false";

pub fn postgres_operation(call: &Call<'_>) -> Option<Operation> {
    let sql = match call.method {
        CREATE_COLLATION => create_collation(call)?,
        REMOVE_COLLATION => format!("DROP COLLATION {}", quoted(&string_argument(call, 0, "name")?)),
        VALIDATE_CONSTRAINT => {
            return Some(Operation::Statement {
                model: Some(model_name(call, 0, "model_name")?),
                sql: format!(
                    "ALTER TABLE {TABLE_PLACEHOLDER} VALIDATE CONSTRAINT {}",
                    quoted(&string_argument(call, 1, "name")?)
                ),
            });
        }
        ALTER_TABLE_COMMENT => {
            let comment = string_argument(call, 1, "table_comment").unwrap_or_default();
            return Some(Operation::Statement {
                model: Some(model_name(call, 0, "name")?),
                sql: format!("COMMENT ON TABLE {TABLE_PLACEHOLDER} IS {}", sql_string(&comment)),
            });
        }
        GENERIC_EXTENSION => return string_argument(call, 0, "name").map(Operation::CreateExtension),
        method => {
            return NAMED_EXTENSIONS
                .iter()
                .find(|(class, _)| *class == method)
                .map(|(_, name)| Operation::CreateExtension((*name).to_owned()));
        }
    };
    Some(Operation::Statement { model: None, sql })
}

fn create_collation(call: &Call<'_>) -> Option<String> {
    let name = string_argument(call, 0, "name")?;
    let mut options = vec![format!("locale={}", quoted(&string_argument(call, 1, "locale")?))];
    let provider = call
        .keyword("provider")
        .or_else(|| call.positional(2))
        .and_then(|node| text(node, call.source))
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_owned());
    if provider != DEFAULT_PROVIDER {
        options.push(format!("provider={}", quoted(&provider)));
    }
    let deterministic = call
        .keyword("deterministic")
        .or_else(|| call.positional(3))
        .and_then(boolean);
    if deterministic == Some(false) {
        options.push(format!("deterministic={NON_DETERMINISTIC}"));
    }
    Some(format!(
        "CREATE COLLATION {} ({})",
        quoted(&name),
        options.join(", ")
    ))
}
