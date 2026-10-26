use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::fix::Fix;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, BODY_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, LIST, RIGHT_FIELD,
    named, text, unparenthesized,
};

const LIST_COMPREHENSION: &str = "list_comprehension";
const GENERATOR: &str = "generator_expression";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const LIST_SPLAT: &str = "list_splat";
const ASYNC: &str = "async";
const ITERATORS: [&str; 6] = ["iter", "enumerate", "filter", "map", "reversed", "zip"];

// El arreglo de RUF015: `next(iter(x))`, o `next(...)` directo si lo de adentro ya es un iterador o una
// comprensión con condiciones; inseguro, como en ruff.
pub fn first_element_fix(
    model: &Model<'_>,
    source: &[u8],
    expression: Node<'_>,
    value: Node<'_>,
) -> Option<Fix> {
    let (iterable, is_iterator) = iteration_target(model, source, unparenthesized(value))?;
    let argument = if is_iterator {
        iterable
    } else {
        format!("iter({iterable})")
    };
    Some(Fix::replace(expression, &format!("next({argument})"), source).marked_unsafe())
}

fn iteration_target(model: &Model<'_>, source: &[u8], value: Node<'_>) -> Option<(String, bool)> {
    match value.kind() {
        CALL => {
            let arguments = value.child_by_field_name(ARGUMENTS_FIELD)?;
            if arguments.kind() == GENERATOR {
                return Some(comprehension_target(arguments, source));
            }
            let given: Vec<Node<'_>> = named(arguments)
                .into_iter()
                .filter(|argument| argument.kind() != COMMENT)
                .collect();
            let [argument] = given.as_slice() else {
                return None;
            };
            Some(match argument.kind() {
                LIST_COMPREHENSION | GENERATOR => comprehension_target(*argument, source),
                CALL => (
                    text(*argument, source).to_owned(),
                    is_iterator_call(model, source, *argument),
                ),
                _ => (text(*argument, source).to_owned(), false),
            })
        }
        LIST_COMPREHENSION => Some(comprehension_target(value, source)),
        LIST => {
            let elements = named(value);
            let [splat] = elements.as_slice() else {
                return None;
            };
            let spread = named(*splat)
                .into_iter()
                .next()
                .filter(|_| splat.kind() == LIST_SPLAT)?;
            let is_iterator = spread.kind() == CALL && is_iterator_call(model, source, spread);
            Some((text(spread, source).to_owned(), is_iterator))
        }
        _ => None,
    }
}

// `match_simple_comprehension` de ruff: `[x for x in y]` es `y`; si no, la comprensión sin sus
// corchetes o paréntesis, que ya es un iterador.
fn comprehension_target(comprehension: Node<'_>, source: &[u8]) -> (String, bool) {
    if let Some(iterable) = simple_iterable(comprehension, source) {
        return (text(iterable, source).to_owned(), false);
    }
    let written = text(comprehension, source);
    let inner = written.get(1..written.len().saturating_sub(1)).unwrap_or(written);
    (inner.to_owned(), true)
}

fn simple_iterable<'tree>(comprehension: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let parts: Vec<Node<'tree>> = named(comprehension)
        .into_iter()
        .filter(|part| part.kind() != COMMENT)
        .collect();
    let [body, clause] = parts.as_slice() else {
        return None;
    };
    let is_async = (0..clause.child_count())
        .filter_map(|index| clause.child(index))
        .any(|child| child.kind() == ASYNC);
    let target = clause.child_by_field_name(LEFT_FIELD)?;
    let same_name = body.kind() == IDENTIFIER
        && target.kind() == IDENTIFIER
        && text(*body, source) == text(target, source);
    (clause.kind() == FOR_IN_CLAUSE
        && !is_async
        && same_name
        && comprehension.child_by_field_name(BODY_FIELD).is_some())
    .then(|| clause.child_by_field_name(RIGHT_FIELD))
    .flatten()
}

fn is_iterator_call(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    call.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
        ITERATORS
            .iter()
            .any(|name| is_builtin_named(model, function, source, name))
    })
}
