use tree_sitter::Node;

use super::if_blocks::{Comparison, operator_is, single_comparison};
use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    COMPARISON, SUBSCRIPT, SUBSCRIPT_FIELD, TUPLE, VALUE_FIELD, fields, is_bytes, is_f_string, is_string,
    named, text, unparenthesized,
};
use crate::lint::python::container_checks::integer_value;
use crate::lint::python::escapes::text_value;
use crate::rules::PythonRule;

const CONDITION_FIELD: &str = "condition";
const OPERATOR_FIELD: &str = "operator";
const ELIF: &str = "elif_clause";
const ELSE: &str = "else_clause";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const INTEGER: &str = "integer";
const SLICE: &str = "slice";
const SLICE_SEPARATOR: char = ':';
const VERSION_INFO: &str = "sys.version_info";
const PLATFORM: &str = "sys.platform";
const PLATFORMS: [&str; 4] = ["linux", "win32", "cygwin", "darwin"];
const LESS: &str = "<";
const EQUAL: &str = "==";
const NOT_EQUAL: &str = "!=";
const MAJOR_TUPLE: i128 = 1;
const MAJOR_MINOR_TUPLE: i128 = 2;
const MAJOR_MINOR: usize = 2;

// Contra qué se compara `sys.version_info`: un número (`[0]`), una tupla de uno (`[:1]`), de dos
// (`[:2]`) o de cualquier largo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    MajorDigit,
    MajorTuple,
    MajorMinorTuple,
    AnyTuple,
}

// Las condiciones de un `if` en un stub: PYI002 (condición que no es una comparación simple con
// `sys.version_info` o `sys.platform`), PYI003, PYI004 y PYI005 (comparaciones de versión que los
// chequeadores no entienden), PYI007 y PYI008 (`sys.platform`) y PYI066 (la rama de la versión vieja
// primero, cuando hay `else`). Cada parte de un `and`/`or` cuenta por separado.
pub fn stub_condition_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    let alternatives: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|child| matches!(child.kind(), ELIF | ELSE))
        .collect();
    let has_else = alternatives.iter().any(|clause| clause.kind() == ELSE);
    let tests = statement.child_by_field_name(CONDITION_FIELD).into_iter().chain(
        alternatives
            .iter()
            .filter(|clause| clause.kind() == ELIF)
            .filter_map(|clause| clause.child_by_field_name(CONDITION_FIELD)),
    );
    let mut reports = Vec::new();
    for test in tests {
        for part in parts(unparenthesized(test), source) {
            let comparison = single_comparison(part, source);
            if let Some(comparison) = comparison.as_ref() {
                reports.extend(version_reports(model, source, part, comparison));
            }
            reports.extend(
                comparison
                    .as_ref()
                    .and_then(|comparison| platform_report(model, source, part, comparison)),
            );
            if is_complex(model, source, part) {
                reports.push(Report::at(PythonRule::PYI002, part, ""));
            }
            let older_first = comparison.as_ref().is_some_and(|comparison| {
                has_else
                    && operator_is(comparison, LESS)
                    && qualified_name(model, comparison.left, source).as_deref() == Some(VERSION_INFO)
            });
            if older_first {
                reports.push(Report::at(PythonRule::PYI066, part, ""));
            }
        }
    }
    reports
}

// Las partes de un `and` o un `or`, con las cadenas del mismo operador aplanadas como en ruff.
fn parts<'tree>(test: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    if test.kind() != BOOLEAN_OPERATOR {
        return vec![test];
    }
    let operator = test
        .child_by_field_name(OPERATOR_FIELD)
        .map(|operator| text(operator, source));
    let mut found = Vec::new();
    for side in named(test) {
        let side = unparenthesized(side);
        let same = side.kind() == BOOLEAN_OPERATOR
            && side
                .child_by_field_name(OPERATOR_FIELD)
                .map(|inner| text(inner, source))
                == operator;
        if same {
            found.extend(parts(side, source));
        } else {
            found.push(side);
        }
    }
    found
}

fn is_complex(model: &Model<'_>, source: &[u8], part: Node<'_>) -> bool {
    if part.kind() != COMPARISON {
        return true;
    }
    let Some(comparison) = single_comparison(part, source) else {
        return true;
    };
    let left = unparenthesized(comparison.left);
    left.kind() != SUBSCRIPT
        && !qualified_name(model, left, source).is_some_and(|name| name == VERSION_INFO || name == PLATFORM)
}

fn version_reports(
    model: &Model<'_>,
    source: &[u8],
    part: Node<'_>,
    comparison: &Comparison<'_>,
) -> Vec<Report> {
    let left = unparenthesized(comparison.left);
    let base = if left.kind() == SUBSCRIPT {
        left.child_by_field_name(VALUE_FIELD).unwrap_or(left)
    } else {
        left
    };
    if qualified_name(model, base, source).as_deref() != Some(VERSION_INFO) {
        return Vec::new();
    }
    let unrecognized = || vec![Report::at(PythonRule::PYI003, part, "")];
    let Some(expected) = expected_comparator(left, source) else {
        return unrecognized();
    };
    let right = unparenthesized(comparison.right);
    if expected == Expected::MajorDigit {
        return if right.kind() == INTEGER {
            Vec::new()
        } else {
            unrecognized()
        };
    }
    if right.kind() != TUPLE {
        return unrecognized();
    }
    let elements = named(right);
    let mut reports = if !elements.iter().all(|element| element.kind() == INTEGER) {
        unrecognized()
    } else if elements.len() > MAJOR_MINOR {
        vec![Report::at(PythonRule::PYI004, part, "")]
    } else {
        Vec::new()
    };
    let expected_length = match expected {
        Expected::MajorTuple => 1,
        Expected::MajorMinorTuple => MAJOR_MINOR,
        Expected::MajorDigit | Expected::AnyTuple => return reports,
    };
    let equality = operator_is(comparison, EQUAL) || operator_is(comparison, NOT_EQUAL);
    if equality && elements.len() != expected_length {
        reports.push(Report::at(PythonRule::PYI005, part, ""));
    }
    reports
}

// `ExpectedComparator::try_from`: sólo `[:1]`, `[:2]` o `[0]`; `None` si el subíndice es otro.
fn expected_comparator(left: Node<'_>, source: &[u8]) -> Option<Expected> {
    if left.kind() != SUBSCRIPT {
        return Some(Expected::AnyTuple);
    }
    let slices = fields(left, SUBSCRIPT_FIELD);
    let [slice] = slices.as_slice() else {
        return None;
    };
    let slice = *slice;
    if slice.kind() == INTEGER {
        return (integer_value(slice, source) == Some(0)).then_some(Expected::MajorDigit);
    }
    if slice.kind() != SLICE {
        return None;
    }
    let written = text(slice, source);
    let (lower, upper) = written.split_once(SLICE_SEPARATOR)?;
    let upper_node = named(slice).into_iter().next()?;
    if !lower.trim().is_empty() || upper.contains(SLICE_SEPARATOR) || upper_node.kind() != INTEGER {
        return None;
    }
    match integer_value(upper_node, source)? {
        MAJOR_TUPLE => Some(Expected::MajorTuple),
        MAJOR_MINOR_TUPLE => Some(Expected::MajorMinorTuple),
        _ => None,
    }
}

// PYI007 (una comparación con `sys.platform` que no es `==` ni `!=` contra un texto) y PYI008 (un
// nombre de plataforma desconocido, que avisa en el texto).
fn platform_report(
    model: &Model<'_>,
    source: &[u8],
    part: Node<'_>,
    comparison: &Comparison<'_>,
) -> Option<Report> {
    if qualified_name(model, unparenthesized(comparison.left), source).as_deref() != Some(PLATFORM) {
        return None;
    }
    if !operator_is(comparison, EQUAL) && !operator_is(comparison, NOT_EQUAL) {
        return Some(Report::at(PythonRule::PYI007, part, ""));
    }
    let right = unparenthesized(comparison.right);
    if !is_string(right) || is_f_string(right, source) || is_bytes(right, source) {
        return Some(Report::at(PythonRule::PYI007, part, ""));
    }
    let platform = text_value(right, source)?;
    (!PLATFORMS.contains(&platform.as_str())).then(|| Report::at(PythonRule::PYI008, right, ""))
}
