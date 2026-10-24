use tree_sitter::Node;

use super::Visitor;
use super::annotations::Annotations;
use super::annotations::embedded::{Placement, wrap};
use super::model::Model;
use super::qualified::{is_typing, qualified_name};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CONCATENATED_STRING, PARENTHESIZED, STRING, SUBSCRIPT, SUBSCRIPT_FIELD, TUPLE, TYPE_FIELD, VALUE_FIELD,
    fields, is_bytes, is_f_string, named, text, unparenthesized,
};
use crate::lint::python::escapes::text_value;
use crate::lint::python::imports::stdlib::STANDARD_LIBRARY;
use crate::rules::PythonRule;
use crate::syntax::{SnippetLanguage, parse_snippet};

const TYPED_DEFAULT_PARAMETER: &str = "typed_default_parameter";
const NONE_LITERAL: &str = "none";
const TYPE_NODE: &str = "type";
const GENERIC_TYPE: &str = "generic_type";
const UNION_TYPE: &str = "union_type";
const BINARY_OPERATOR: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const BIT_OR: &str = "|";
const BUILTINS_PREFIX: &str = "builtins.";
const OBJECT: &str = "builtins.object";
const HASHABLE: &str = "collections.abc.Hashable";
const OPTIONAL: &str = "Optional";
const LITERAL: &str = "Literal";
const UNION: &str = "Union";
const ANNOTATED: &str = "Annotated";
const ANY: &str = "Any";
const TYPING_HASHABLE: &str = "Hashable";

// `TypingTarget` de ruff. `AllowsNone` junta `None`, `Optional`, `Any`, `object`, `Hashable` y lo
// que no se sabe qué es (un alias o una clase del proyecto, que ruff toma como `Any`).
enum Target<'tree> {
    AllowsNone,
    Known,
    Literal(Vec<Node<'tree>>),
    Union(Vec<Node<'tree>>),
    Annotated(Option<Node<'tree>>),
    Forward(Node<'tree>),
}

#[derive(Clone, Copy)]
struct Context<'model, 'tree, 'source> {
    model: &'model Model<'tree>,
    source: &'source [u8],
}

// RUF013: un parámetro con `= None` cuyo tipo no admite `None` (el `Optional` implícito que prohíbe
// PEP 484). Avisa en la parte de la anotación que habría que cambiar, también adentro de comillas.
pub fn implicit_optional_reports(visitor: &Visitor<'_>, parameters: &[Node<'_>]) -> Vec<Report> {
    parameters
        .iter()
        .filter(|parameter| parameter.kind() == TYPED_DEFAULT_PARAMETER)
        .filter(|parameter| {
            parameter
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| unparenthesized(value).kind() == NONE_LITERAL)
        })
        .filter_map(|parameter| parameter.child_by_field_name(TYPE_FIELD))
        .filter_map(|annotation| offending_annotation(visitor, annotation))
        .collect()
}

fn offending_annotation(visitor: &Visitor<'_>, annotation: Node<'_>) -> Option<Report> {
    let annotation = unwrapped(annotation);
    let context = Context {
        model: &visitor.model,
        source: visitor.source,
    };
    if !matches!(annotation.kind(), STRING | CONCATENATED_STRING) {
        return explicit(annotation, context).map(|node| Report::at(PythonRule::RUF013, node, ""));
    }
    let parts = if annotation.kind() == STRING {
        vec![annotation]
    } else {
        named(annotation)
    };
    if parts
        .iter()
        .any(|part| is_f_string(*part, visitor.source) || is_bytes(*part, visitor.source))
    {
        return None;
    }
    let contents: Vec<_> = parts.iter().map(|part| visitor.content(*part)).collect();
    let placement = Placement {
        simple: matches!(contents.as_slice(), [only] if only.simple),
        origin: contents
            .first()
            .map_or_else(|| annotation.start_position(), |first| first.origin),
        fallback: annotation.start_position(),
    };
    let value: String = contents.into_iter().map(|content| content.value).collect();
    let wrapped = wrap(&value);
    let tree = parse_snippet(SnippetLanguage::Python, &wrapped)?;
    let expression = parsed_expression(tree.root_node())?;
    let snippet = Context {
        source: wrapped.as_bytes(),
        ..context
    };
    explicit(expression, snippet).map(|node| {
        Report::at_point(
            PythonRule::RUF013,
            placement.wrapped().map(node.start_position()),
            "",
        )
    })
}

fn parsed_expression(root: Node<'_>) -> Option<Node<'_>> {
    if root.has_error() {
        return None;
    }
    let statement = named(root).into_iter().next()?;
    let wrapper = named(statement).into_iter().next()?;
    Some(unwrapped(wrapper))
}

// `type_hint_explicitly_allows_none`: la parte de la anotación que no admite `None`, si la hay.
fn explicit<'tree>(node: Node<'tree>, context: Context<'_, '_, '_>) -> Option<Node<'tree>> {
    match classify(node, context) {
        Target::AllowsNone => None,
        Target::Annotated(first) => first.and_then(|first| explicit(first, context)),
        target @ (Target::Known | Target::Literal(_) | Target::Union(_) | Target::Forward(_)) => {
            (!contains_none(&target, context)).then_some(node)
        }
    }
}

fn contains_none(target: &Target<'_>, context: Context<'_, '_, '_>) -> bool {
    match target {
        Target::AllowsNone => true,
        Target::Known => false,
        Target::Literal(elements) => elements
            .iter()
            .any(|element| unwrapped(*element).kind() == NONE_LITERAL),
        Target::Union(elements) => elements
            .iter()
            .any(|element| contains_none(&classify(*element, context), context)),
        Target::Annotated(first) => {
            first.is_some_and(|first| contains_none(&classify(first, context), context))
        }
        Target::Forward(string) => forward_contains_none(*string, context),
    }
}

fn forward_contains_none(string: Node<'_>, context: Context<'_, '_, '_>) -> bool {
    let Some(value) = text_value(string, context.source) else {
        return true;
    };
    let wrapped = wrap(&value);
    let Some(tree) = parse_snippet(SnippetLanguage::Python, &wrapped) else {
        return true;
    };
    let snippet = Context {
        source: wrapped.as_bytes(),
        ..context
    };
    parsed_expression(tree.root_node())
        .is_none_or(|expression| contains_none(&classify(expression, snippet), snippet))
}

fn classify<'tree>(node: Node<'tree>, context: Context<'_, '_, '_>) -> Target<'tree> {
    let node = unwrapped(node);
    match node.kind() {
        NONE_LITERAL => Target::AllowsNone,
        STRING | CONCATENATED_STRING => Target::Forward(node),
        SUBSCRIPT => {
            let slices: Vec<Node<'tree>> = fields(node, SUBSCRIPT_FIELD)
                .into_iter()
                .flat_map(|slice| {
                    let slice = unparenthesized(slice);
                    if slice.kind() == TUPLE {
                        named(slice)
                    } else {
                        vec![slice]
                    }
                })
                .collect();
            subscript_target(node.child_by_field_name(VALUE_FIELD), slices, context)
        }
        GENERIC_TYPE => {
            let parts = named(node);
            let slices = parts
                .iter()
                .skip(1)
                .flat_map(|parameters| named(*parameters))
                .collect();
            subscript_target(parts.first().copied(), slices, context)
        }
        UNION_TYPE => Target::Union(named(node)),
        BINARY_OPERATOR
            if node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, context.source) == BIT_OR) =>
        {
            Target::Union(named(node))
        }
        _ => name_target(node, context),
    }
}

fn subscript_target<'tree>(
    value: Option<Node<'_>>,
    slices: Vec<Node<'tree>>,
    context: Context<'_, '_, '_>,
) -> Target<'tree> {
    let Some(name) = value.and_then(|value| qualified_name(context.model, value, context.source)) else {
        return Target::AllowsNone;
    };
    if is_typing(&name, OPTIONAL) {
        Target::AllowsNone
    } else if is_typing(&name, LITERAL) {
        Target::Literal(slices)
    } else if is_typing(&name, UNION) {
        Target::Union(slices)
    } else if is_typing(&name, ANNOTATED) {
        Target::Annotated(slices.first().copied())
    } else if is_known_type(&name) {
        Target::Known
    } else {
        Target::AllowsNone
    }
}

fn name_target<'tree>(node: Node<'_>, context: Context<'_, '_, '_>) -> Target<'tree> {
    let Some(name) = qualified_name(context.model, node, context.source) else {
        return Target::AllowsNone;
    };
    let allows_any =
        is_typing(&name, ANY) || name == OBJECT || is_typing(&name, TYPING_HASHABLE) || name == HASHABLE;
    if !allows_any && is_known_type(&name) {
        Target::Known
    } else {
        Target::AllowsNone
    }
}

// `is_known_type`: un builtin o algo de la biblioteca estándar.
fn is_known_type(name: &str) -> bool {
    let module = name.split('.').next().unwrap_or_default();
    name.starts_with(BUILTINS_PREFIX) || STANDARD_LIBRARY.binary_search(&module).is_ok()
}

fn unwrapped(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while matches!(current.kind(), TYPE_NODE | PARENTHESIZED) {
        match named(current).first() {
            Some(inner) => current = *inner,
            None => break,
        }
    }
    current
}
