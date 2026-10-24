use tree_sitter::Node;

use super::if_blocks::single_comparison;
use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    SUBSCRIPT_FIELD, VALUE_FIELD, fields, is_bytes, is_f_string, is_string, named, text, unparenthesized,
};
use crate::lint::python::container_checks::integer_value;
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const VERSION: &str = "sys.version";
const MINOR: &str = "sys.version_info.minor";
const SIX_PY3: &str = "six.PY3";
const INTEGER: &str = "integer";
const SLICE: &str = "slice";
const SLICE_SEPARATOR: char = ':';
const ORDERINGS: [&str; 4] = ["<", "<=", ">", ">="];
const MAJOR_SLICE: i128 = 1;
const MAJOR_MINOR_SLICE: i128 = 3;
const MINOR_INDEX: i128 = 2;
const MAJOR_INDEX: i128 = 0;

// YTT101, YTT102, YTT301 y YTT303: `sys.version[:3]`, `sys.version[2]`, `sys.version[0]` y
// `sys.version[:1]`, que se rompen con Python 3.10 o 10. Avisa en `sys.version`.
pub fn version_subscript(model: &Model<'_>, source: &[u8], subscript: Node<'_>) -> Option<Report> {
    let value = subscript.child_by_field_name(VALUE_FIELD)?;
    if qualified_name(model, value, source).as_deref() != Some(VERSION) {
        return None;
    }
    let slices = fields(subscript, SUBSCRIPT_FIELD);
    let [slice] = slices.as_slice() else {
        return None;
    };
    let rule = match slice.kind() {
        INTEGER => match integer_value(*slice, source)? {
            MINOR_INDEX => PythonRule::YTT102,
            MAJOR_INDEX => PythonRule::YTT301,
            _ => return None,
        },
        SLICE => match upper_bound(*slice, source)? {
            MAJOR_SLICE => PythonRule::YTT303,
            MAJOR_MINOR_SLICE => PythonRule::YTT101,
            _ => return None,
        },
        _ => return None,
    };
    Some(Report::at(rule, value, ""))
}

// El `n` de `[:n]`, sin inicio ni paso.
fn upper_bound(slice: Node<'_>, source: &[u8]) -> Option<i128> {
    let (lower, upper) = text(slice, source).split_once(SLICE_SEPARATOR)?;
    let bound = named(slice).into_iter().next()?;
    (lower.trim().is_empty() && !upper.contains(SLICE_SEPARATOR) && bound.kind() == INTEGER)
        .then(|| integer_value(bound, source))
        .flatten()
}

// YTT204 (`sys.version_info.minor < 7`), YTT103 y YTT302 (`sys.version` comparado con un texto, de un
// carácter o más). Avisa en el lado izquierdo.
pub fn version_comparison(model: &Model<'_>, source: &[u8], comparison: Node<'_>) -> Option<Report> {
    let parts = single_comparison(comparison, source)?;
    let [operator] = parts.operator.as_slice() else {
        return None;
    };
    if !ORDERINGS.contains(operator) {
        return None;
    }
    let left = unparenthesized(parts.left);
    let right = unparenthesized(parts.right);
    let rule = match qualified_name(model, left, source).as_deref() {
        Some(MINOR) if right.kind() == INTEGER => PythonRule::YTT204,
        Some(VERSION) if is_string(right) && !is_f_string(right, source) && !is_bytes(right, source) => {
            if text_value(right, source)?.chars().count() == 1 {
                PythonRule::YTT302
            } else {
                PythonRule::YTT103
            }
        }
        _ => return None,
    };
    Some(Report::at(rule, left, ""))
}

// YTT202: `six.PY3`, que deja de ser verdadero con Python 4.
pub fn is_six_py3(name: &str) -> bool {
    name == SIX_PY3
}
