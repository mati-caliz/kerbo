use tree_sitter::Node;

use super::model::Model;
use super::qualified::qualified_name;
use crate::lint::python::Report;
use crate::lint::python::ast::{CALL, CLASS, COMMENT, FUNCTION_FIELD, named, unparenthesized};
use crate::lint::python::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;

const CACHES: [&str; 2] = ["functools.lru_cache", "functools.cache"];
const SUPERCLASSES_FIELD: &str = "superclasses";
const ENUM_MODULE: &str = "enum.";

// B019: `@functools.cache` o `@lru_cache` en un método de instancia, que guarda `self` en la caché
// y no lo libera nunca. Las enumeraciones quedan afuera, como en ruff.
pub fn cached_instance_method(
    model: &Model<'_>,
    source: &[u8],
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Vec<Report> {
    if method_kind(function, source) != Some(MethodKind::Instance)
        || is_in_enumeration(model, source, function)
    {
        return Vec::new();
    }
    decorators
        .iter()
        .filter(|decorator| {
            named(**decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
                .map(|expression| callable(unparenthesized(expression)))
                .and_then(|callable| qualified_name(model, callable, source))
                .is_some_and(|name| CACHES.contains(&name.as_str()))
        })
        .map(|decorator| Report::at(PythonRule::B019, *decorator, ""))
        .collect()
}

fn callable(expression: Node<'_>) -> Node<'_> {
    if expression.kind() == CALL {
        expression
            .child_by_field_name(FUNCTION_FIELD)
            .unwrap_or(expression)
    } else {
        expression
    }
}

fn is_in_enumeration(model: &Model<'_>, source: &[u8], function: Node<'_>) -> bool {
    std::iter::successors(function.parent(), Node::parent)
        .find(|ancestor| ancestor.kind() == CLASS)
        .and_then(|class| class.child_by_field_name(SUPERCLASSES_FIELD))
        .is_some_and(|bases| {
            named(bases).into_iter().any(|base| {
                qualified_name(model, base, source).is_some_and(|name| name.starts_with(ENUM_MODULE))
            })
        })
}
