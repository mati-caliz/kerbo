use tree_sitter::Node;

use super::if_blocks::{Comparison, is_version_info, operator_is, single_comparison};
use super::model::Model;
use super::version_block_fix::{Verdict, outdated_block_fix};
use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, TUPLE, named, unparenthesized};
use crate::lint::python::container_checks::integer_value;
use crate::rules::PythonRule;

const INTEGER: &str = "integer";
const LESS: &str = "<";
const LESS_EQUAL: &str = "<=";
const GREATER: &str = ">";
const GREATER_EQUAL: &str = ">=";
const EQUAL: &str = "==";
const NOT_EQUAL: &str = "!=";
const TARGET_MAJOR: u8 = 3;
const TARGET_MINOR: u8 = 11;
const PYTHON_TWO: u8 = 2;

enum Outcome {
    Outdated(Verdict),
    Invalid,
    Current,
    Abort,
}

// UP036 para cada rama del `if`, con la versión mínima por defecto de ruff 0.17 (3.11). Como ruff,
// una tupla que no es toda de enteros o un entero que no es 2 ni 3 cortan el resto de las ramas.
pub fn outdated_version_blocks(
    model: &Model<'_>,
    source: &[u8],
    statement: Node<'_>,
    tests: &[Node<'_>],
) -> Vec<Report> {
    let mut reports = Vec::new();
    for (branch, test) in tests.iter().enumerate() {
        let Some(comparison) = single_comparison(*test, source) else {
            continue;
        };
        if !is_version_info(model, source, comparison.left) {
            continue;
        }
        let right = unparenthesized(comparison.right);
        let outcome = match right.kind() {
            TUPLE => tuple_outcome(&comparison, right, source),
            INTEGER => integer_outcome(&comparison, right, source),
            _ => Outcome::Current,
        };
        match outcome {
            Outcome::Outdated(verdict) => {
                let fix = outdated_block_fix(source, statement, branch, verdict);
                reports.push(Report::at(PythonRule::UP036, unparenthesized(*test), "").with_fix(fix));
            }
            Outcome::Invalid => reports.push(Report::at(PythonRule::UP036, comparison.right, "")),
            Outcome::Current => {}
            Outcome::Abort => break,
        }
    }
    reports
}

fn small(value: i128) -> Option<u8> {
    u8::try_from(value).ok()
}

fn tuple_outcome(comparison: &Comparison<'_>, tuple: Node<'_>, source: &[u8]) -> Outcome {
    let ordering = [LESS, LESS_EQUAL, GREATER, GREATER_EQUAL]
        .into_iter()
        .find(|operator| operator_is(comparison, operator));
    let Some(ordering) = ordering else {
        return Outcome::Current;
    };
    let elements: Vec<Node<'_>> = named(tuple)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect();
    let mut version = Vec::new();
    for element in elements {
        let Some(number) = (element.kind() == INTEGER)
            .then(|| integer_value(element, source))
            .flatten()
        else {
            return Outcome::Abort;
        };
        version.push(number);
    }
    let or_equal = ordering == LESS_EQUAL || ordering == GREATER;
    match always_less_than(&version, or_equal) {
        Some(true) if ordering == LESS || ordering == LESS_EQUAL => Outcome::Outdated(Verdict::AlwaysFalse),
        Some(true) => Outcome::Outdated(Verdict::AlwaysTrue),
        Some(false) => Outcome::Current,
        None => Outcome::Invalid,
    }
}

// `version_always_less_than` de ruff: `None` es una versión inválida.
fn always_less_than(version: &[i128], or_equal: bool) -> Option<bool> {
    let Some(major) = version.first() else {
        return Some(false);
    };
    let major = small(*major)?;
    if major != TARGET_MAJOR {
        return Some(major < TARGET_MAJOR);
    }
    let Some(minor) = version.get(1) else {
        return Some(true);
    };
    let minor = small(*minor)?;
    let micro = match version.get(2) {
        Some(micro) => Some(small(*micro)?),
        None => None,
    };
    Some(if or_equal {
        minor < TARGET_MINOR
    } else if let Some(micro) = micro {
        minor < TARGET_MINOR || (minor == TARGET_MINOR && micro == 0)
    } else {
        minor <= TARGET_MINOR
    })
}

// Un entero sólo vale como versión mayor: 2 o 3 con un operador de orden o de igualdad.
fn integer_outcome(comparison: &Comparison<'_>, integer: Node<'_>, source: &[u8]) -> Outcome {
    let Some(major) = integer_value(integer, source).and_then(small) else {
        return Outcome::Invalid;
    };
    let operator = [EQUAL, NOT_EQUAL, LESS, LESS_EQUAL, GREATER, GREATER_EQUAL]
        .into_iter()
        .find(|operator| operator_is(comparison, operator));
    match operator {
        Some(operator) if major == PYTHON_TWO || major == TARGET_MAJOR => {
            Outcome::Outdated(major_version_verdict(major == TARGET_MAJOR, operator))
        }
        _ => Outcome::Abort,
    }
}

// La tabla de ruff para `sys.version_info[0]` contra 2 o 3, con 3 como versión mayor mínima.
fn major_version_verdict(is_current_major: bool, operator: &str) -> Verdict {
    let always_true = match operator {
        EQUAL | LESS_EQUAL => is_current_major,
        NOT_EQUAL | GREATER => !is_current_major,
        GREATER_EQUAL => true,
        _ => false,
    };
    if always_true {
        Verdict::AlwaysTrue
    } else {
        Verdict::AlwaysFalse
    }
}
