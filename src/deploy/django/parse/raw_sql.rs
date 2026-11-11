use tree_sitter::Node;

use super::{CONCATENATION, Scope, argument};
use crate::deploy::django::computed_names::name_text;
use crate::deploy::django::operations::{Operation, SqlChunk};
use crate::deploy::django::run_python::schema_editor_sql;
use crate::deploy::django::values::{elements, is_sequence};
use crate::deploy::python_calls::Call;
use crate::syntax::one_based;

const NOOP: &str = "noop";

pub(super) fn run_python<'context>(scope: Scope<'context>, call: &Call<'context>) -> Option<Operation> {
    let name = argument(call, 0, "code")?.utf8_text(scope.source).ok()?;
    let function = scope
        .functions
        .iter()
        .rev()
        .find(|(defined, _)| defined == name)
        .map(|(_, function)| *function)?;
    let chunks = schema_editor_sql(function, scope.source);
    (!chunks.is_empty()).then_some(Operation::RunSql(chunks))
}

pub(super) fn run_sql<'context>(scope: Scope<'context>, call: &Call<'context>) -> Option<Operation> {
    let sql = scope.resolved(argument(call, 0, "sql")?);
    let is_noop = sql.kind() == "attribute"
        && sql
            .utf8_text(scope.source)
            .is_ok_and(|reference| reference.ends_with(NOOP));
    if is_noop {
        return None;
    }
    let chunks = sql_literals(scope, sql)
        .into_iter()
        .map(|literal| scope.resolved(literal))
        .filter_map(|literal| {
            Some(SqlChunk {
                sql: name_text(literal, scope.constants)?,
                line: one_based(literal.start_position().row),
            })
        })
        .collect();
    Some(Operation::RunSql(chunks))
}

fn sql_literals<'context>(scope: Scope<'context>, sql: Node<'context>) -> Vec<Node<'context>> {
    if !is_sequence(sql) && sql.kind() != CONCATENATION {
        return vec![sql];
    }
    scope
        .sequence(sql)
        .into_iter()
        .filter_map(|element| {
            let element = scope.resolved(element);
            if is_sequence(element) {
                elements(element).into_iter().next()
            } else {
                Some(element)
            }
        })
        .collect()
}
