use tree_sitter::Node;

use super::qualified::{is_typing, qualified_name};
use super::type_hints::Hints;
use crate::lint::python::ast::{CALL, COMMENT, DECORATED, FUNCTION_FIELD, named, unparenthesized};

const DECORATOR: &str = "decorator";
const OVERLOAD: &str = "overload";
const FINAL: &str = "final";
const ABSTRACT_DECORATORS: [&str; 4] = [
    "abc.abstractmethod",
    "abc.abstractproperty",
    "abc.abstractclassmethod",
    "abc.abstractstaticmethod",
];

// Los nombres calificados de los decoradores, sin los argumentos si son una llamada.
pub fn qualified_decorators(hints: Hints<'_, '_, '_>, decorators: &[Node<'_>]) -> Vec<String> {
    decorators
        .iter()
        .filter(|decorator| decorator.kind() == DECORATOR)
        .filter_map(|decorator| {
            named(*decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
        })
        .filter_map(|expression| {
            qualified_name(hints.model, callable(unparenthesized(expression)), hints.source)
        })
        .collect()
}

// Los decoradores de una función o clase, si está decorada.
pub fn decorators_of(definition: Node<'_>) -> Vec<Node<'_>> {
    definition
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .collect()
}

pub fn is_abstract(hints: Hints<'_, '_, '_>, decorators: &[Node<'_>]) -> bool {
    qualified_decorators(hints, decorators)
        .iter()
        .any(|name| ABSTRACT_DECORATORS.contains(&name.as_str()))
}

pub fn is_overload(hints: Hints<'_, '_, '_>, decorators: &[Node<'_>]) -> bool {
    qualified_decorators(hints, decorators)
        .iter()
        .any(|name| is_typing(name, OVERLOAD))
}

pub fn is_final(hints: Hints<'_, '_, '_>, decorators: &[Node<'_>]) -> bool {
    qualified_decorators(hints, decorators)
        .iter()
        .any(|name| is_typing(name, FINAL))
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
