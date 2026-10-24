use tree_sitter::Node;

use super::qualified::qualified_name;
use super::type_hints::Hints;
use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, named, unparenthesized};
use crate::lint::python::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;

const DECORATOR: &str = "decorator";
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
        .map(|decorator| Report::at(rule, *decorator, ""))
        .collect()
}
