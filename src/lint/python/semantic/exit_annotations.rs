use tree_sitter::Node;

use super::call_fixes::builtin_fix;
use super::classes::DEFINITION_FIELD;
use super::decorators::{decorators_of, is_overload};
use super::parameters::{Signature, annotation, has_default, matches_annotation};
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BLOCK, BODY_FIELD, CLASS, DECORATED, FUNCTION, LEFT_FIELD, NAME_FIELD, NONE, RIGHT_FIELD, named, text,
};
use crate::rules::PythonRule;

const EXIT: &str = "__exit__";
const ASYNC_EXIT: &str = "__aexit__";
const ASYNC: &str = "async";
const EXCEPTION_ARGUMENTS: usize = 3;
const OVERLOADS: usize = 2;
const BINARY_OPERATOR: &str = "binary_operator";
const UNION_TYPE: &str = "union_type";
const OPERATOR_FIELD: &str = "operator";
const BIT_OR: &str = "|";
const OPTIONAL: &str = "Optional";
const UNION: &str = "Union";
const TYPE: &str = "Type";
const TYPE_BUILTIN: &str = "type";
const BASE_EXCEPTION: &str = "BaseException";
const OBJECT: &str = "object";
const UNUSED: &str = "_typeshed.Unused";
const TRACEBACK: &str = "types.TracebackType";

type Predicate = fn(Hints<'_, '_, '_>, Node<'_>) -> bool;

const EXPECTED: [Predicate; EXCEPTION_ARGUMENTS] = [is_base_exception_type, is_base_exception, is_traceback];

// PYI036: la firma de `__exit__` o `__aexit__` de una clase, con sus tres argumentos de la
// excepción (o `*args: object`) y el resto con valor por defecto.
pub fn exit_annotation_reports(
    hints: Hints<'_, '_, '_>,
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Vec<Report> {
    let Some(name) = exit_name(hints, function) else {
        return Vec::new();
    };
    let Some(class) = parent_class(function) else {
        return Vec::new();
    };
    let signature = Signature::of(function);
    let arguments: Vec<Node<'_>> = signature.positional.iter().skip(1).copied().collect();
    let report = |node: Node<'_>| Report::at(PythonRule::PYI036, node, name);
    if is_overload(hints, decorators) {
        let unrecognized = unrecognized_overload(hints, class, name, &arguments);
        return signature
            .node
            .filter(|_| unrecognized)
            .map(report)
            .into_iter()
            .collect();
    }
    let mut reports = Vec::new();
    if arguments.len() < EXCEPTION_ARGUMENTS {
        match signature.variadic {
            Some(variadic) => reports.extend(
                annotation(variadic)
                    .filter(|annotation| !matches_annotation(hints, *annotation, &is_object_or_unused))
                    .map(|annotation| {
                        report(annotation).with_fix(builtin_fix(
                            hints.model,
                            annotation,
                            OBJECT,
                            hints.source,
                        ))
                    }),
            ),
            None => reports.extend(signature.node.map(report)),
        }
    }
    let without_default = arguments
        .iter()
        .skip(EXCEPTION_ARGUMENTS)
        .chain(&signature.keyword_only)
        .filter(|parameter| !has_default(**parameter));
    reports.extend(without_default.map(|parameter| report(*parameter)));
    for (parameter, expected) in arguments.iter().zip(EXPECTED) {
        let Some(annotation) = annotation(*parameter) else {
            continue;
        };
        let accepted = matches_annotation(hints, annotation, &is_object_or_unused)
            || matches_annotation(hints, annotation, &|inner, expression| {
                non_none_element(inner, expression).is_some_and(|element| expected(inner, element))
            });
        if !accepted {
            reports.push(report(annotation));
        }
    }
    reports
}

fn exit_name(hints: Hints<'_, '_, '_>, function: Node<'_>) -> Option<&'static str> {
    let name = text(function.child_by_field_name(NAME_FIELD)?, hints.source);
    let is_async = function
        .child(0)
        .is_some_and(|first| text(first, hints.source) == ASYNC);
    match (name, is_async) {
        (EXIT, false) => Some(EXIT),
        (ASYNC_EXIT, true) => Some(ASYNC_EXIT),
        _ => None,
    }
}

// La clase cuando la función es una sentencia directa de su cuerpo.
fn parent_class(function: Node<'_>) -> Option<Node<'_>> {
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    holder
        .parent()
        .filter(|body| body.kind() == BLOCK)?
        .parent()
        .filter(|class| class.kind() == CLASS)
}

// Con exactamente dos sobrecargas de tres argumentos cada una, la anotada tiene que ser `None, None,
// None` o `type[BaseException], BaseException, TracebackType`.
fn unrecognized_overload(
    hints: Hints<'_, '_, '_>,
    class: Node<'_>,
    name: &str,
    arguments: &[Node<'_>],
) -> bool {
    let overloads: Vec<Node<'_>> = class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|statement| {
            if statement.kind() == DECORATED {
                statement.child_by_field_name(DEFINITION_FIELD)
            } else {
                Some(statement)
            }
        })
        .filter(|function| function.kind() == FUNCTION)
        .filter(|function| has_name(hints, *function, name))
        .filter(|function| is_overload(hints, &decorators_of(*function)))
        .collect();
    let comparable = overloads.len() == OVERLOADS
        && overloads.iter().all(|function| {
            let signature = Signature::of(*function);
            exit_name(hints, *function) == Some(name)
                && signature.variadic.is_none()
                && signature.keywords.is_none()
                && signature.keyword_only.is_empty()
                && signature.positional.len() == EXCEPTION_ARGUMENTS + 1
        });
    if !comparable {
        return false;
    }
    let loosely = |parameter: &Node<'_>, expected: Predicate| {
        annotation(*parameter).is_none_or(|annotation| {
            matches_annotation(hints, annotation, &|inner, expression| {
                expected(inner, expression) || is_object_or_unused(inner, expression)
            })
        })
    };
    let all_none = arguments.iter().all(|parameter| loosely(parameter, is_none));
    let exception = arguments.len() == EXCEPTION_ARGUMENTS
        && arguments
            .iter()
            .zip(EXPECTED)
            .all(|(parameter, expected)| loosely(parameter, expected));
    !all_none && !exception
}

fn has_name(hints: Hints<'_, '_, '_>, function: Node<'_>, name: &str) -> bool {
    function
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|identifier| text(identifier, hints.source) == name)
}

// `non_none_annotation_element`: `X` en `Optional[X]`, `Union[X, None]` o `X | None`.
fn non_none_element<'tree>(hints: Hints<'_, '_, '_>, annotation: Node<'tree>) -> Option<Node<'tree>> {
    if let Some((value, slices)) = subscript_parts(annotation) {
        let name = qualified_name(hints.model, value, hints.source)?;
        if is_typing(&name, OPTIONAL) {
            return slices
                .first()
                .map(|slice| unwrapped(*slice))
                .filter(|slice| !is_none(hints, *slice));
        }
        if !is_typing(&name, UNION) {
            return None;
        }
        return match slices.as_slice() {
            [left, right] => match (is_none(hints, *left), is_none(hints, *right)) {
                (false, true) => Some(unwrapped(*left)),
                (true, false) => Some(unwrapped(*right)),
                _ => None,
            },
            _ => None,
        };
    }
    let (left, right) = union_sides(hints, annotation)?;
    if is_none(hints, left) {
        Some(right).filter(|right| !is_none(hints, *right))
    } else {
        Some(left)
    }
}

// Los dos lados de `X | Y`, en una expresión o en una anotación.
fn union_sides<'tree>(hints: Hints<'_, '_, '_>, node: Node<'tree>) -> Option<(Node<'tree>, Node<'tree>)> {
    let sides = match node.kind() {
        UNION_TYPE => named(node),
        BINARY_OPERATOR
            if node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, hints.source) == BIT_OR) =>
        {
            [LEFT_FIELD, RIGHT_FIELD]
                .into_iter()
                .filter_map(|side| node.child_by_field_name(side))
                .collect()
        }
        _ => return None,
    };
    match sides.as_slice() {
        [left, right] => Some((unwrapped(*left), unwrapped(*right))),
        _ => None,
    }
}

fn is_none(_hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    unwrapped(node).kind() == NONE
}

fn is_object_or_unused(hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    is_builtin_named(hints.model, node, hints.source, OBJECT)
        || qualified_name(hints.model, node, hints.source).is_some_and(|name| name == UNUSED)
}

fn is_traceback(hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    qualified_name(hints.model, node, hints.source).is_some_and(|name| name == TRACEBACK)
}

fn is_base_exception(hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    is_builtin_named(hints.model, node, hints.source, BASE_EXCEPTION)
}

fn is_base_exception_type(hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    let Some((value, slices)) = subscript_parts(node) else {
        return false;
    };
    let is_type = is_builtin_named(hints.model, value, hints.source, TYPE_BUILTIN)
        || qualified_name(hints.model, value, hints.source).is_some_and(|name| is_typing(&name, TYPE));
    is_type && matches!(slices.as_slice(), [only] if is_base_exception(hints, unwrapped(*only)))
}
