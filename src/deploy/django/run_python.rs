use tree_sitter::Node;

use super::names::quoted;
use super::operations::SqlChunk;
use super::values::text;
use crate::deploy::python_calls::Call;
use crate::syntax::{descendants, named_children};

const FUNCTION_DEFINITION: &str = "function_definition";
const ALTER_DB_TABLE: &str = "alter_db_table";
const OLD_TABLE_POSITION: usize = 1;
const NEW_TABLE_POSITION: usize = 2;

pub fn functions<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<(String, Node<'tree>)> {
    named_children(root)
        .into_iter()
        .filter(|node| node.kind() == FUNCTION_DEFINITION)
        .filter_map(|function| {
            let name = function.child_by_field_name("name")?.utf8_text(source).ok()?;
            Some((name.to_owned(), function))
        })
        .collect()
}

pub fn schema_editor_sql(function: Node<'_>, source: &[u8]) -> Vec<SqlChunk> {
    descendants(function)
        .into_iter()
        .filter_map(|node| Call::parse(node, source))
        .filter(|call| call.method == ALTER_DB_TABLE)
        .filter_map(|call| {
            let old_table = call
                .positional(OLD_TABLE_POSITION)
                .and_then(|node| text(node, source))?;
            let new_table = call
                .positional(NEW_TABLE_POSITION)
                .and_then(|node| text(node, source))?;
            (old_table != new_table).then(|| SqlChunk {
                sql: format!(
                    "ALTER TABLE {} RENAME TO {}",
                    quoted(&old_table),
                    quoted(&new_table)
                ),
                line: call.line,
            })
        })
        .collect()
}
