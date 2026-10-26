use tree_sitter::Node;

use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::qualified::qualified_name;
use super::type_hints::Hints;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, named, unparenthesized};
use crate::lint::python::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;

const DECORATOR: &str = "decorator";
const FUNCTOOLS: &str = "functools";
const SINGLEDISPATCH_NAME: &str = "singledispatch";
const SINGLEDISPATCHMETHOD_NAME: &str = "singledispatchmethod";
const SINGLEDISPATCH: &str = "functools.singledispatch";
const SINGLEDISPATCHMETHOD: &str = "functools.singledispatchmethod";

// PLE1519 (`@singledispatch` en un método) y PLE1520 (`@singledispatchmethod` en una función suelta).
// Avisan en el decorador.
pub fn dispatch_decorator_reports(
    hints: Hints<'_, '_, '_>,
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Vec<Report> {
    let (wrong, rule) = match method_kind(function, hints.source) {
        Some(MethodKind::Instance | MethodKind::Class | MethodKind::Static) => {
            (SINGLEDISPATCH, PythonRule::PLE1519)
        }
        Some(MethodKind::New) => return Vec::new(),
        None => (SINGLEDISPATCHMETHOD, PythonRule::PLE1520),
    };
    decorators
        .iter()
        .filter(|decorator| decorator.kind() == DECORATOR)
        .filter(|decorator| {
            named(**decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
                .and_then(|expression| qualified_name(hints.model, unparenthesized(expression), hints.source))
                .is_some_and(|name| name == wrong)
        })
        .map(|decorator| {
            let expression = named(*decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT);
            let fix = expression.and_then(|expression| dispatch_fix(hints, expression, rule));
            Report::at(rule, *decorator, "").with_fix(fix)
        })
        .collect()
}

// El arreglo de PLE1519 y PLE1520: el decorador que corresponde de `functools`; inseguro, como en ruff.
fn dispatch_fix(hints: Hints<'_, '_, '_>, expression: Node<'_>, rule: PythonRule) -> Option<Fix> {
    let member = if rule == PythonRule::PLE1519 {
        SINGLEDISPATCHMETHOD_NAME
    } else {
        SINGLEDISPATCH_NAME
    };
    let request = ImportRequest {
        module: FUNCTOOLS,
        member,
        style: ImportStyle::Import,
    };
    let (binding, import) = get_or_import_member(hints.model, hints.source, expression, request)?;
    let reference = TextEdit {
        start: TextPoint::from_point(expression.start_position()),
        end: TextPoint::from_point(expression.end_position()),
        replacement: binding,
    };
    Some(Fix::from_edits(hints.source, vec![reference, import]).marked_unsafe())
}
