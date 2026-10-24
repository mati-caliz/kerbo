use tree_sitter::Node;

use super::Mode;
use super::qualified::{is_builtin_named, is_typing, qualified_name};
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, BODY_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, NAME_FIELD, is_string,
    named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;
use crate::syntax::{SnippetLanguage, parse_snippet};

const PARAMETERS_FIELD: &str = "parameters";
const RETURN_TYPE_FIELD: &str = "return_type";
const TYPE_FIELD: &str = "type";
const DECORATOR: &str = "decorator";
const ASYNC: &str = "async";
const NONE_LITERAL: &str = "none";
const POSITIONAL_SEPARATOR: &str = "positional_separator";
const KEYWORD_SEPARATOR: &str = "keyword_separator";
const LIST_SPLAT_PATTERN: &str = "list_splat_pattern";
const DICTIONARY_SPLAT_PATTERN: &str = "dictionary_splat_pattern";
const TYPED: [&str; 2] = ["typed_parameter", "typed_default_parameter"];
const POSITIONAL: [&str; 4] = [
    IDENTIFIER,
    "typed_parameter",
    "default_parameter",
    "typed_default_parameter",
];
const EQUALITY_METHODS: [&str; 2] = ["__eq__", "__ne__"];
const STRING_METHODS: [&str; 2] = ["__str__", "__repr__"];
const ITER: &str = "__iter__";
const AITER: &str = "__aiter__";
const GENERATORS: [&str; 2] = ["typing.Generator", "collections.abc.Generator"];
const ASYNC_GENERATORS: [&str; 2] = ["typing.AsyncGenerator", "collections.abc.AsyncGenerator"];
const ABSTRACT_DECORATORS: [&str; 4] = [
    "abc.abstractmethod",
    "abc.abstractproperty",
    "abc.abstractclassmethod",
    "abc.abstractstaticmethod",
];
const ANY: &str = "Any";
const NO_RETURN: &str = "NoReturn";
const NUMERIC: [&str; 3] = ["int", "float", "complex"];
const STR: &str = "str";
const DUNDER: &str = "__";
const REGISTER: &str = "register";
const SIMPLE_STATEMENTS: [&str; 2] = ["expression_statement", "return_statement"];

#[derive(Clone, Copy)]
struct Signature<'list, 'tree> {
    function: Node<'tree>,
    name: &'list str,
    decorators: &'list [Node<'tree>],
}

// Las reglas de `flake8-pyi` sobre la firma de una función: PYI050 (`NoReturn` en un parámetro),
// PYI041 (`int | float` en un parámetro), PYI032 (`Any` en `__eq__`), PYI063 (`__x` como parámetro
// sólo posicional), PYI058 (`__iter__` que devuelve `Generator`) y PYI029 (`__str__` repetido en un
// stub).
pub fn signature_reports(
    hints: Hints<'_, '_, '_>,
    function: Node<'_>,
    decorators: &[Node<'_>],
    stub: bool,
) -> Vec<Report> {
    let source = hints.source;
    let Some(name) = function
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
    else {
        return Vec::new();
    };
    let signature = Signature {
        function,
        name,
        decorators,
    };
    let parameters = parameters(function);
    let mut reports = Vec::new();
    for annotation in parameters
        .iter()
        .filter(|parameter| TYPED.contains(&parameter.kind()))
        .filter_map(|parameter| parameter.child_by_field_name(TYPE_FIELD))
    {
        if typing_named(hints, annotation, NO_RETURN) || quoted_typing_named(hints, annotation, NO_RETURN) {
            reports.push(Report::at(PythonRule::PYI050, annotation, ""));
        }
        if !is_dispatch_registration(decorators, source) && is_redundant_numeric_union(hints, annotation) {
            reports.push(Report::at(PythonRule::PYI041, annotation, ""));
        }
    }
    let kind = method_kind(function, source);
    reports.extend(any_equality_annotation(hints, signature, &parameters, kind));
    reports.extend(old_style_positional_only(source, &parameters, kind));
    reports.extend(generator_from_iter(hints, signature, &parameters, kind));
    if stub {
        reports.extend(redundant_string_method(hints, signature, &parameters, kind));
    }
    reports
}

fn parameters(function: Node<'_>) -> Vec<Node<'_>> {
    function
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| parameter.kind() != COMMENT)
        .collect()
}

// Los parámetros posicionales (`args` de ruff): hasta el `*`; `None` si hay un `/`.
fn positional<'tree>(parameters: &[Node<'tree>]) -> Option<Vec<Node<'tree>>> {
    if parameters
        .iter()
        .any(|parameter| parameter.kind() == POSITIONAL_SEPARATOR)
    {
        return None;
    }
    Some(
        parameters
            .iter()
            .take_while(|parameter| !is_variadic_or_separator(**parameter))
            .filter(|parameter| POSITIONAL.contains(&parameter.kind()))
            .copied()
            .collect(),
    )
}

fn is_variadic_or_separator(parameter: Node<'_>) -> bool {
    parameter.kind() == KEYWORD_SEPARATOR
        || splat(parameter).is_some_and(|inner| inner.kind() == LIST_SPLAT_PATTERN)
}

fn splat(parameter: Node<'_>) -> Option<Node<'_>> {
    if matches!(parameter.kind(), LIST_SPLAT_PATTERN | DICTIONARY_SPLAT_PATTERN) {
        return Some(parameter);
    }
    named(parameter)
        .into_iter()
        .find(|child| matches!(child.kind(), LIST_SPLAT_PATTERN | DICTIONARY_SPLAT_PATTERN))
}

fn has_variadics(parameters: &[Node<'_>]) -> bool {
    parameters
        .iter()
        .any(|parameter| parameter.kind() == KEYWORD_SEPARATOR || splat(*parameter).is_some())
}

fn parameter_name<'tree>(parameter: Node<'tree>, source: &'tree [u8]) -> Option<&'tree str> {
    let name = if parameter.kind() == IDENTIFIER {
        Some(parameter)
    } else {
        parameter.child_by_field_name(NAME_FIELD).or_else(|| {
            named(parameter)
                .into_iter()
                .find(|child| child.kind() == IDENTIFIER)
        })
    };
    name.map(|name| text(name, source))
}

fn typing_named(hints: Hints<'_, '_, '_>, annotation: Node<'_>, member: &str) -> bool {
    qualified_name(hints.model, unwrapped(annotation), hints.source)
        .is_some_and(|name| is_typing(&name, member))
}

// `quoted_annotation` de ruff para un nombre suelto: `'NoReturn'` entre comillas.
fn quoted_typing_named(hints: Hints<'_, '_, '_>, annotation: Node<'_>, member: &str) -> bool {
    let annotation = unwrapped(annotation);
    if !is_string(annotation) {
        return false;
    }
    let Some(value) = text_value(annotation, hints.source) else {
        return false;
    };
    let Some(tree) = parse_snippet(SnippetLanguage::Python, value.trim()) else {
        return false;
    };
    let root = tree.root_node();
    let expression = named(root)
        .into_iter()
        .next()
        .and_then(|statement| named(statement).into_iter().next());
    !root.has_error()
        && expression.is_some_and(|expression| {
            qualified_name(hints.model, expression, value.trim().as_bytes())
                .is_some_and(|name| is_typing(&name, member))
        })
}

// `@f.register` de `functools.singledispatch`, donde la anotación elige la sobrecarga.
fn is_dispatch_registration(decorators: &[Node<'_>], source: &[u8]) -> bool {
    decorators
        .iter()
        .filter_map(|decorator| {
            named(*decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
        })
        .map(|expression| callable(unparenthesized(expression)))
        .any(|callable| {
            callable.kind() == ATTRIBUTE
                && callable
                    .child_by_field_name(ATTRIBUTE_FIELD)
                    .is_some_and(|attribute| text(attribute, source) == REGISTER)
        })
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

fn is_redundant_numeric_union(hints: Hints<'_, '_, '_>, annotation: Node<'_>) -> bool {
    if !hints.is_union(annotation, Mode::TypeDefinition) {
        return false;
    }
    let mut members = Vec::new();
    hints.union_members(annotation, Mode::TypeDefinition, &mut members);
    let has = |builtin: &str| {
        members
            .iter()
            .any(|member| is_builtin_named(hints.model, *member, hints.source, builtin))
    };
    let [int, float, complex] = NUMERIC.map(has);
    if complex { float || int } else { float && int }
}

// PYI032: `__eq__` o `__ne__` cuyo segundo parámetro está anotado con `Any`.
fn any_equality_annotation(
    hints: Hints<'_, '_, '_>,
    signature: Signature<'_, '_>,
    parameters: &[Node<'_>],
    kind: Option<MethodKind>,
) -> Option<Report> {
    if !EQUALITY_METHODS.contains(&signature.name) || kind.is_none() {
        return None;
    }
    let positional = positional(parameters)?;
    let [_, other] = positional.as_slice() else {
        return None;
    };
    let annotation = other.child_by_field_name(TYPE_FIELD)?;
    typing_named(hints, annotation, ANY).then(|| Report::at(PythonRule::PYI032, annotation, ""))
}

// PYI063: el primer parámetro (después de `self` o `cls`) escrito `__x`, el sólo posicional de PEP
// 484, cuando ya existe `/`.
fn old_style_positional_only(
    source: &[u8],
    parameters: &[Node<'_>],
    kind: Option<MethodKind>,
) -> Option<Report> {
    let positional = positional(parameters)?;
    let skip = usize::from(matches!(kind, Some(MethodKind::Instance | MethodKind::Class)));
    let parameter = positional.get(skip)?;
    let name = parameter_name(*parameter, source)?;
    (name.starts_with(DUNDER) && !name.ends_with(DUNDER))
        .then(|| Report::at(PythonRule::PYI063, *parameter, ""))
}

// PYI058: un `__iter__` (o `__aiter__`) que sólo recibe `self` y devuelve `Generator` (o
// `AsyncGenerator`) sin usar lo que se envía ni lo que devuelve. Avisa en el nombre.
fn generator_from_iter(
    hints: Hints<'_, '_, '_>,
    signature: Signature<'_, '_>,
    parameters: &[Node<'_>],
    kind: Option<MethodKind>,
) -> Option<Report> {
    let (generators, unused_arguments) = match signature.name {
        ITER => (GENERATORS, 2_usize),
        AITER => (ASYNC_GENERATORS, 1),
        _ => return None,
    };
    let is_async = signature
        .function
        .child(0)
        .is_some_and(|first| text(first, hints.source) == ASYNC);
    if is_async
        || kind.is_none()
        || has_variadics(parameters)
        || positional(parameters)?.len() != 1
        || !has_single_simple_statement(signature.function)
    {
        return None;
    }
    let returns = unwrapped(signature.function.child_by_field_name(RETURN_TYPE_FIELD)?);
    let (value, slices) = subscript_parts(returns).unwrap_or((returns, Vec::new()));
    let name = qualified_name(hints.model, value, hints.source)?;
    if !generators.contains(&name.as_str()) {
        return None;
    }
    let unused = slices
        .iter()
        .skip(1)
        .all(|slice| unwrapped(*slice).kind() == NONE_LITERAL || typing_named(hints, *slice, ANY));
    let complete = slices.is_empty() || slices.len() == unused_arguments.saturating_add(1);
    (unused && complete).then(|| {
        Report::at(
            PythonRule::PYI058,
            signature
                .function
                .child_by_field_name(NAME_FIELD)
                .unwrap_or(signature.function),
            "",
        )
    })
}

// Lo que ruff toma como `__iter__` simple: una sola sentencia que no es compuesta (un `yield`, un
// `yield from` o un `return`).
fn has_single_simple_statement(function: Node<'_>) -> bool {
    let statements: Vec<Node<'_>> = function
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|statement| statement.kind() != COMMENT)
        .collect();
    matches!(statements.as_slice(), [only] if SIMPLE_STATEMENTS.contains(&only.kind()))
}

// PYI029: un `__str__` o `__repr__` en un stub con la misma firma que el de `object`.
fn redundant_string_method(
    hints: Hints<'_, '_, '_>,
    signature: Signature<'_, '_>,
    parameters: &[Node<'_>],
    kind: Option<MethodKind>,
) -> Option<Report> {
    if !STRING_METHODS.contains(&signature.name) || kind.is_none() {
        return None;
    }
    let keyword_only = parameters
        .iter()
        .skip_while(|parameter| !is_variadic_or_separator(**parameter))
        .skip(1)
        .any(|parameter| POSITIONAL.contains(&parameter.kind()) && splat(*parameter).is_none());
    if keyword_only || positional(parameters)?.len() > 1 || is_abstract(hints, signature.decorators) {
        return None;
    }
    let returns = signature.function.child_by_field_name(RETURN_TYPE_FIELD)?;
    is_builtin_named(hints.model, unwrapped(returns), hints.source, STR).then(|| {
        Report::at(
            PythonRule::PYI029,
            signature
                .function
                .child_by_field_name(NAME_FIELD)
                .unwrap_or(signature.function),
            "",
        )
    })
}

fn is_abstract(hints: Hints<'_, '_, '_>, decorators: &[Node<'_>]) -> bool {
    decorators
        .iter()
        .filter(|decorator| decorator.kind() == DECORATOR)
        .filter_map(|decorator| {
            named(*decorator)
                .into_iter()
                .find(|child| child.kind() != COMMENT)
        })
        .map(|expression| callable(unparenthesized(expression)))
        .any(|callable| {
            qualified_name(hints.model, callable, hints.source)
                .is_some_and(|name| ABSTRACT_DECORATORS.contains(&name.as_str()))
        })
}
