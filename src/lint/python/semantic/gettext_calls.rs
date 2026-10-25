use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, CALL, CONCATENATED_STRING, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD,
    OBJECT_FIELD, STRING, is_bytes, is_f_string, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const GETTEXT_FUNCTIONS: [&str; 3] = ["_", "gettext", "ngettext"];
const BUILTINS_PREFIX: &str = "builtins.";
const QUALIFIED_GETTEXT: [&str; 2] = ["gettext.gettext", "gettext.ngettext"];
const FORMAT: &str = "format";
const BINARY_OPERATOR: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const PERCENT: &str = "%";

// El texto que se pasa a `gettext` ya formateado, así que el catálogo busca el texto completo: INT001
// (un f-string), INT002 (`"...".format(x)`) e INT003 (`"..." % x`). Sólo el primer argumento: el
// plural de `ngettext` es de preview.
pub fn gettext_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Option<Report> {
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    if !is_gettext(model, source, function) {
        return None;
    }
    let first = arguments(call).positional.first().copied()?;
    let rule = if is_f_string_expression(first, source) {
        PythonRule::INT001
    } else if is_format_call(first, source) {
        PythonRule::INT002
    } else if is_printf_format(first, source) {
        PythonRule::INT003
    } else {
        return None;
    };
    Some(Report::at(rule, first, ""))
}

// `is_gettext_func_call` de ruff: `_`, `gettext` o `ngettext` por su nombre, sin mirar de dónde vienen,
// o `gettext.gettext` y `gettext.ngettext` importados con cualquier alias.
fn is_gettext(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    if function.kind() == IDENTIFIER && GETTEXT_FUNCTIONS.contains(&text(function, source)) {
        return true;
    }
    qualified_name(model, function, source).is_some_and(|name| {
        QUALIFIED_GETTEXT.contains(&name.as_str())
            || name
                .strip_prefix(BUILTINS_PREFIX)
                .is_some_and(|builtin| function.kind() == ATTRIBUTE && GETTEXT_FUNCTIONS.contains(&builtin))
    })
}

fn is_f_string_expression(node: Node<'_>, source: &[u8]) -> bool {
    match node.kind() {
        STRING => is_f_string(node, source),
        CONCATENATED_STRING => named(node)
            .into_iter()
            .any(|part| part.kind() == STRING && is_f_string(part, source)),
        _ => false,
    }
}

fn is_format_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == CALL
        && node
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE)
            .is_some_and(|function| {
                function
                    .child_by_field_name(ATTRIBUTE_FIELD)
                    .is_some_and(|attribute| text(attribute, source) == FORMAT)
                    && function.child_by_field_name(OBJECT_FIELD).is_some()
            })
}

// `"..." % x` con un string común a la izquierda; ni bytes ni f-strings.
fn is_printf_format(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == BINARY_OPERATOR
        && node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == PERCENT)
        && node
            .child_by_field_name(LEFT_FIELD)
            .map(unparenthesized)
            .is_some_and(|left| {
                matches!(left.kind(), STRING | CONCATENATED_STRING)
                    && !is_f_string_expression(left, source)
                    && !is_bytes(left, source)
            })
}
