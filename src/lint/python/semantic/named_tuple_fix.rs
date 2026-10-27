use tree_sitter::Node;

use super::builtin_calls::arguments;
use crate::fix::Fix;
use crate::lint::python::ast::{
    COMMENT, FUNCTION_FIELD, KEYWORD_ARGUMENT, LEFT_FIELD, NAME_FIELD, VALUE_FIELD, named, text,
    unparenthesized,
};
use crate::lint::python::codegen;
use crate::lint::python::escapes::text_value;
use crate::lint::python::fix_edits::{has_comment_within, indentation_unit};

const LIST: &str = "list";
const PASS: &str = "pass";
const DUNDER: &str = "__";

// El `convert_to_class` de ruff: `Point = NamedTuple("Point", [("x", int)])` pasa a una clase con un
// campo anotado por elemento, impresa con el generador de ruff. Sólo si la sentencia empieza en la
// primera columna (ruff no conserva la sangría) e inseguro si tiene comentarios adentro.
pub fn named_tuple_class_fix(source: &[u8], statement: Node<'_>, call: Node<'_>) -> Option<Fix> {
    if statement.start_position().column != 0 {
        return None;
    }
    let target = statement.child_by_field_name(LEFT_FIELD)?;
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let given = arguments(call);
    let fields = match (given.positional.as_slice(), given.keywords.is_empty()) {
        ([_], true) => Vec::new(),
        ([_, fields], true) => listed_fields(*fields, source)?,
        ([_], false) => keyword_fields(&given.keywords, source)?,
        _ => return None,
    };
    let indentation = indentation_unit(statement, source);
    let body = if fields.is_empty() {
        format!("{indentation}{PASS}")
    } else {
        fields
            .iter()
            .map(|(name, annotation)| format!("{indentation}{name}: {annotation}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let class = format!(
        "class {}({}):\n{body}",
        text(target, source),
        codegen::unparse(function, codegen::COMMA, source)
    );
    let fix = Fix::replace(statement, &class, source);
    Some(
        if has_comment_within(statement, statement.start_byte(), statement.end_byte()) {
            fix.marked_unsafe()
        } else {
            fix
        },
    )
}

fn listed_fields(fields: Node<'_>, source: &[u8]) -> Option<Vec<(String, String)>> {
    let fields = unparenthesized(fields);
    if fields.kind() != LIST {
        return None;
    }
    without_comments(fields)
        .into_iter()
        .map(|field| {
            let parts = without_comments(field);
            let [name, annotation] = parts.as_slice() else {
                return None;
            };
            let name = text_value(*name, source)?;
            if name.starts_with(DUNDER) && name.ends_with(DUNDER) {
                return None;
            }
            Some((name, codegen::unparse(*annotation, codegen::COMMA, source)))
        })
        .collect()
}

fn keyword_fields(keywords: &[Node<'_>], source: &[u8]) -> Option<Vec<(String, String)>> {
    keywords
        .iter()
        .map(|keyword| {
            if keyword.kind() != KEYWORD_ARGUMENT {
                return None;
            }
            let name = keyword.child_by_field_name(NAME_FIELD)?;
            let value = keyword.child_by_field_name(VALUE_FIELD)?;
            Some((
                text(name, source).to_owned(),
                codegen::unparse(value, codegen::COMMA, source),
            ))
        })
        .collect()
}

fn without_comments(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}
