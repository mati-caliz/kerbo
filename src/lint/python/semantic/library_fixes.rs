use tree_sitter::Node;

use super::builtin_calls::{arguments, keyword_value};
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::Model;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{
    ATTRIBUTE, CALL, FUNCTION_FIELD, LEFT_FIELD, OBJECT_FIELD, SUBSCRIPT, VALUE_FIELD, string_prefix, text,
    unparenthesized,
};
use crate::lint::python::fix_edits::{commented_unsafe, delete_between, pad, point_of};
use crate::rules::PythonRule;

const BINARY_OPERATOR_KIND: &str = "binary_operator";
const PATHLIB: &str = "pathlib";
const PATH: &str = "Path";
const SUFFIX: &str = "suffix";
const DOT: &str = ".";
const HEXDIGEST: &str = ".hexdigest";

// Los arreglos de SIM911, FURB177, FURB181 y PTH210, que kerbo detecta sobre la llamada entera.
pub fn library_fix(rule: PythonRule, model: &Model<'_>, call: Node<'_>, source: &[u8]) -> Option<Fix> {
    let given = &arguments(call);
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    match (rule, given.positional.first().copied()) {
        (PythonRule::SIM911, Some(keys)) => {
            let keys = unparenthesized(keys);
            let mapping = keys
                .child_by_field_name(FUNCTION_FIELD)?
                .child_by_field_name(OBJECT_FIELD)?;
            let replacement = pad(format!("{}.items()", text(mapping, source)), call, source);
            Some(commented_unsafe(
                Fix::replace(call, &replacement, source),
                call,
                source,
            ))
        }
        (PythonRule::FURB177, _) => {
            let request = ImportRequest {
                module: PATHLIB,
                member: PATH,
                style: ImportStyle::ImportFrom,
            };
            let (binding, import) = get_or_import_member(model, source, call, request)?;
            let reference = TextEdit {
                start: TextPoint::from_point(call.start_position()),
                end: TextPoint::from_point(call.end_position()),
                replacement: format!("{binding}.cwd()"),
            };
            Some(Fix::from_edits(source, vec![reference, import]).marked_unsafe())
        }
        (PythonRule::FURB181, _) => {
            let digest = function.child_by_field_name(OBJECT_FIELD)?;
            let hash = digest
                .child_by_field_name(FUNCTION_FIELD)?
                .child_by_field_name(OBJECT_FIELD)?;
            let fix = Fix::from_edits(
                source,
                vec![TextEdit {
                    start: point_of(source, hash.end_byte()),
                    end: point_of(source, function.end_byte()),
                    replacement: HEXDIGEST.to_owned(),
                }],
            );
            Some(commented_unsafe(fix, call, source))
        }
        (PythonRule::PTH210, first) => {
            let suffix = keyword_value(given, SUFFIX, source).or(first)?;
            let string = unparenthesized(suffix);
            if text(string, source)
                .get(string_prefix(string, source).len()..)?
                .get(1..2)
                == Some(DOT)
            {
                return None;
            }
            let opener = string_prefix(string, source).len().saturating_add(1);
            let at = string.start_byte().saturating_add(opener);
            Some(
                Fix::from_edits(
                    source,
                    vec![TextEdit {
                        start: point_of(source, at),
                        end: point_of(source, at),
                        replacement: DOT.to_owned(),
                    }],
                )
                .marked_unsafe(),
            )
        }
        _ => None,
    }
}

// FURB162: se borra desde el final de la fecha hasta el final del argumento (el `.replace("Z", ...)`,
// el `.rstrip("Z") + ...` o el `[:-1] + ...`); inseguro, como en ruff.
pub fn utc_suffix_fix(argument: Node<'_>, source: &[u8]) -> Option<Fix> {
    let mut value = unparenthesized(argument);
    if value.kind() == BINARY_OPERATOR_KIND {
        value = value.child_by_field_name(LEFT_FIELD)?;
        value = unparenthesized(value);
    }
    let date = match value.kind() {
        CALL => value
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE)?
            .child_by_field_name(OBJECT_FIELD)?,
        SUBSCRIPT => value.child_by_field_name(VALUE_FIELD)?,
        _ => return None,
    };
    Some(delete_between(date.end_byte(), argument.end_byte(), source).marked_unsafe())
}
