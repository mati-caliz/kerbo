use std::collections::HashSet;

use tree_sitter::Node;

use super::Pending;
use super::classes::{DEFINITION_FIELD, any_base, bases};
use super::decorators::{decorators_of, is_abstract, is_final, is_overload};
use super::definitions::type_parameter_parts;
use super::model::{BindingKind, Model, Resolution};
use super::mutability::{Resolver, without_subscript};
use super::parameters::{Signature, annotation, matches_annotation};
use super::private_types::assigned_call;
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, BODY_FIELD, CALL, CLASS, DECORATED, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LAMBDA,
    NAME_FIELD, named, text,
};
use crate::lint::python::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;

const RETURN_TYPE_FIELD: &str = "return_type";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const ASYNC: &str = "async";
const ASYNC_ENTER: &str = "__aenter__";
const RETURNS_SELF: [&str; 2] = ["__enter__", "__new__"];
const NEW: &str = "__new__";
const ITER: &str = "__iter__";
const AITER: &str = "__aiter__";
const INPLACE_OPERATORS: [&str; 13] = [
    "__iadd__",
    "__isub__",
    "__imul__",
    "__imatmul__",
    "__itruediv__",
    "__ifloordiv__",
    "__imod__",
    "__ipow__",
    "__ilshift__",
    "__irshift__",
    "__iand__",
    "__ixor__",
    "__ior__",
];
const ITERATORS: [&str; 2] = ["typing.Iterator", "collections.abc.Iterator"];
const ITERABLES: [&str; 4] = [
    "typing.Iterable",
    "typing.Iterator",
    "collections.abc.Iterable",
    "collections.abc.Iterator",
];
const ASYNC_ITERATORS: [&str; 2] = ["typing.AsyncIterator", "collections.abc.AsyncIterator"];
const ASYNC_ITERABLES: [&str; 4] = [
    "typing.AsyncIterable",
    "typing.AsyncIterator",
    "collections.abc.AsyncIterable",
    "collections.abc.AsyncIterator",
];
const METACLASSES: [&str; 4] = ["builtins.type", "abc.ABCMeta", "enum.EnumMeta", "enum.EnumType"];
const TYPE_BUILTIN: &str = "type";
const TYPE_VAR: &str = "TypeVar";
const METACLASS_NEW_PARAMETERS: usize = 5;
const METACLASS_NEW_ANNOTATIONS: [&str; 3] = ["str", "tuple", "dict"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Metaclass {
    Yes,
    No,
    Maybe,
}

#[derive(Clone, Copy)]
struct Method<'tree> {
    function: Node<'tree>,
    class: Node<'tree>,
    name: Node<'tree>,
}

// PYI034 sobre un método: el retorno que debería ser `Self`.
pub fn non_self_return_report(
    resolver: &Resolver<'_, '_>,
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Option<Report> {
    let (method, metaclass) = method_of(resolver, function, decorators)?;
    (metaclass == Metaclass::No)
        .then(|| non_self_return(resolver, method))
        .flatten()
}

// PYI019 sobre un método: el `TypeVar` propio para `self` o `cls` donde alcanza con `Self`.
pub fn custom_type_variable_report(
    resolver: &Resolver<'_, '_>,
    function: Node<'_>,
    decorators: &[Node<'_>],
) -> Option<Report> {
    let (method, metaclass) = method_of(resolver, function, decorators)?;
    (metaclass != Metaclass::Yes)
        .then(|| custom_type_variable(resolver, method))
        .flatten()
}

// PYI019 con las anotaciones diferidas (import del futuro o stub): como ruff, el `TypeVar` se resuelve
// al final del módulo, así que puede estar definido después de la clase.
pub fn deferred_type_variables(model: &Model<'_>, source: &[u8], methods: &[Pending<'_>]) -> Vec<Report> {
    methods
        .iter()
        .filter_map(|method| {
            let resolver = Resolver {
                model,
                source,
                scope: method.scope,
            };
            custom_type_variable_report(&resolver, method.node, &decorators_of(method.node))
        })
        .collect()
}

fn method_of<'tree>(
    resolver: &Resolver<'_, '_>,
    function: Node<'tree>,
    decorators: &[Node<'_>],
) -> Option<(Method<'tree>, Metaclass)> {
    let class = enclosing_class(function)?;
    let name = function.child_by_field_name(NAME_FIELD)?;
    let hints = Hints {
        model: resolver.model,
        source: resolver.source,
    };
    if is_abstract(hints, decorators) || is_overload(hints, decorators) {
        return None;
    }
    let method = Method {
        function,
        class,
        name,
    };
    Some((method, metaclass(resolver, class)))
}

// La clase cuando la función está definida en su cuerpo (aunque sea adentro de un `if`).
fn enclosing_class(function: Node<'_>) -> Option<Node<'_>> {
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    std::iter::successors(holder.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA))
        .filter(|owner| owner.kind() == CLASS)
}

// PYI034: `__enter__`, `__new__`, `__aenter__` y los operadores en el lugar que devuelven el nombre
// de la clase, y `__iter__`/`__aiter__` que devuelven un iterador en una clase que ya lo es. Avisa en
// el nombre del método.
fn non_self_return(resolver: &Resolver<'_, '_>, method: Method<'_>) -> Option<Report> {
    let source = resolver.source;
    let hints = Hints {
        model: resolver.model,
        source,
    };
    let signature = Signature::of(method.function);
    let returns = method.function.child_by_field_name(RETURN_TYPE_FIELD)?;
    if signature.positional.is_empty() {
        return None;
    }
    let method_name = text(method.name, source);
    let class_name = text(method.class.child_by_field_name(NAME_FIELD)?, source);
    let is_final_class = is_final(hints, &decorators_of(method.class));
    let returns_class = matches_annotation(hints, returns, &|inner, expression| {
        expression.kind() == IDENTIFIER && text(expression, inner.source) == class_name
    });
    let report = || Report::at(PythonRule::PYI034, method.name, method_name);
    if is_async(method.function, source) {
        return (method_name == ASYNC_ENTER && returns_class && !is_final_class).then(report);
    }
    if INPLACE_OPERATORS.contains(&method_name) {
        return returns_class.then(report);
    }
    if returns_class {
        return (RETURNS_SELF.contains(&method_name) && !is_final_class).then(report);
    }
    let (expected, bases) = match method_name {
        ITER => (ITERABLES, ITERATORS),
        AITER => (ASYNC_ITERABLES, ASYNC_ITERATORS),
        _ => return None,
    };
    let returns_iterator = qualified_name(hints.model, head(returns), source)
        .is_some_and(|name| expected.contains(&name.as_str()));
    let subclasses = any_base(
        resolver,
        method.class,
        &mut |base| {
            resolver
                .qualified(without_subscript(base))
                .is_some_and(|name| bases.contains(&name.as_str()))
        },
        &mut HashSet::new(),
    );
    (returns_iterator && subclasses).then(report)
}

// PYI019: `self: _S` o `cls: type[_S]` con un `TypeVar` propio (o un parámetro de tipo del método).
// Avisa desde el final del nombre.
fn custom_type_variable(resolver: &Resolver<'_, '_>, method: Method<'_>) -> Option<Report> {
    let source = resolver.source;
    let hints = Hints {
        model: resolver.model,
        source,
    };
    let kind = method_kind(method.function, source)?;
    let first = *Signature::of(method.function).positional.first()?;
    let annotation = annotation(first)?;
    let type_parameters = own_type_parameters(method.function, source);
    let found = matches_annotation(hints, annotation, &|inner, expression| {
        let variable = match kind {
            MethodKind::Class | MethodKind::New => class_type_argument(inner, expression),
            MethodKind::Instance => Some(expression).filter(|node| node.kind() == IDENTIFIER),
            MethodKind::Static => None,
        };
        variable.is_some_and(|variable| {
            let name = text(variable, inner.source);
            type_parameters.iter().any(|parameter| parameter == name) || is_type_variable(resolver, name)
        })
    });
    found.then(|| Report::at_point(PythonRule::PYI019, method.name.end_position(), ""))
}

// `_S` en `type[_S]`, con el `type` del builtin escrito tal cual.
fn class_type_argument<'tree>(hints: Hints<'_, '_, '_>, annotation: Node<'tree>) -> Option<Node<'tree>> {
    let (value, slices) = subscript_parts(annotation)?;
    let is_type =
        value.kind() == IDENTIFIER && is_builtin_named(hints.model, value, hints.source, TYPE_BUILTIN);
    match slices.as_slice() {
        [only] if is_type && unwrapped(*only).kind() == IDENTIFIER => Some(unwrapped(*only)),
        _ => None,
    }
}

fn own_type_parameters(function: Node<'_>, source: &[u8]) -> Vec<String> {
    function
        .child_by_field_name(TYPE_PARAMETERS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|parameter| type_parameter_parts(parameter).0)
        .map(|name| text(name, source).to_owned())
        .collect()
}

// Un nombre ligado a `X = TypeVar(...)`.
fn is_type_variable(resolver: &Resolver<'_, '_>, name: &str) -> bool {
    let Resolution::Binding(id) = resolver.model.lookup_from(resolver.scope, name) else {
        return false;
    };
    resolver
        .model
        .binding(id)
        .filter(|binding| binding.kind == BindingKind::Assignment)
        .and_then(|binding| binding.site.node)
        .and_then(assigned_call)
        .and_then(|function| resolver.qualified(function))
        .is_some_and(|qualified| is_typing(&qualified, TYPE_VAR))
}

// `is_metaclass` de ruff: hereda de `type`, `ABCMeta` o `EnumMeta` (o quizás, si alguna base es una
// llamada o el `__new__` tiene la firma del de `type`).
fn metaclass(resolver: &Resolver<'_, '_>, class: Node<'_>) -> Metaclass {
    let mut maybe = false;
    let is_base = any_base(
        resolver,
        class,
        &mut |base| {
            if base.kind() == CALL {
                maybe = true;
                return is_type_call(resolver, base);
            }
            if let Some((value, _)) = subscript_parts(base) {
                return is_builtin_named(resolver.model, value, resolver.source, TYPE_BUILTIN);
            }
            resolver
                .qualified(base)
                .is_some_and(|name| METACLASSES.contains(&name.as_str()))
        },
        &mut HashSet::new(),
    );
    match (is_base, maybe) {
        (true, true) => Metaclass::Maybe,
        (true, false) => Metaclass::Yes,
        _ if !bases(class).is_empty() && has_metaclass_new(resolver, class) => Metaclass::Maybe,
        _ => Metaclass::No,
    }
}

// `type(X)`, que devuelve la metaclase de `X`.
fn is_type_call(resolver: &Resolver<'_, '_>, call: Node<'_>) -> bool {
    let single = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| named(arguments).len() == 1);
    single
        && call
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(resolver.model, function, resolver.source, TYPE_BUILTIN))
}

// Un `__new__(cls, name: str, bases: tuple[...], attrs: dict[...], **kwargs)` en el cuerpo.
fn has_metaclass_new(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    class
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .map(|statement| {
            if statement.kind() == DECORATED {
                statement
                    .child_by_field_name(DEFINITION_FIELD)
                    .unwrap_or(statement)
            } else {
                statement
            }
        })
        .filter(|function| function.kind() == FUNCTION)
        .filter(|function| {
            function
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, resolver.source) == NEW)
        })
        .any(|function| {
            let signature = Signature::of(function);
            if signature.count() != METACLASS_NEW_PARAMETERS
                || signature.variadic.is_some()
                || signature.keywords.is_none()
            {
                return false;
            }
            let ordered: Vec<Node<'_>> = signature
                .positional
                .iter()
                .chain(&signature.keyword_only)
                .skip(1)
                .copied()
                .collect();
            ordered.len() >= METACLASS_NEW_ANNOTATIONS.len()
                && ordered
                    .iter()
                    .zip(METACLASS_NEW_ANNOTATIONS)
                    .all(|(parameter, builtin)| {
                        annotation(*parameter).is_some_and(|annotation| {
                            is_builtin_named(resolver.model, head(annotation), resolver.source, builtin)
                        })
                    })
        })
}

// `map_subscript`: `Iterator[int]` se resuelve como `Iterator`.
fn head(annotation: Node<'_>) -> Node<'_> {
    let annotation = unwrapped(annotation);
    subscript_parts(annotation).map_or(annotation, |(value, _)| value)
}

fn is_async(function: Node<'_>, source: &[u8]) -> bool {
    function
        .child(0)
        .is_some_and(|first| text(first, source) == ASYNC)
}
