use tree_sitter::Node;

use super::classes::bases;
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::{BindingKind, Resolution};
use super::mutability::Resolver;
use super::parameters::{Signature, annotation};
use super::private_types::assigned_call;
use super::qualified::{is_builtin_named, is_typing};
use super::type_hints::{subscript_parts, unwrapped};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{ATTRIBUTE, IDENTIFIER, NAME_FIELD, TYPE_FIELD, named, text};
use crate::lint::python::builtins::is_builtin;

const TYPING: &str = "typing";
const SELF: &str = "Self";
const TYPE_BUILTIN: &str = "type";
const TYPING_TYPE: &str = "Type";
const ANY_STR: &str = "AnyStr";
const RETURN_TYPE_FIELD: &str = "return_type";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const LIST_SPLAT: &str = "list_splat";
const TYPE_VARIABLE_FACTORIES: [&str; 3] = ["TypeVar", "ParamSpec", "TypeVarTuple"];

// El `replace_with_self_fix` de ruff: el retorno pasa a `Self` (con el importer) y el `self: Clase` o
// `cls: type[Clase]` pierde la anotación. Inseguro; si la clase podría ser genérica, ruff sólo lo
// muestra y nunca lo aplica.
pub fn self_return_fix(resolver: &Resolver<'_, '_>, function: Node<'_>, class: Node<'_>) -> Option<Fix> {
    if might_be_generic(resolver, class) {
        return None;
    }
    let source = resolver.source;
    let returns = function.child_by_field_name(RETURN_TYPE_FIELD)?;
    let request = ImportRequest {
        module: TYPING,
        member: SELF,
        style: ImportStyle::ImportFrom,
    };
    let (binding, import) = get_or_import_member(resolver.model, source, returns, request)?;
    let class_name = text(class.child_by_field_name(NAME_FIELD)?, source);
    let mut edits = vec![import];
    edits.extend(first_annotation_removal(resolver, function, class_name));
    edits.push(TextEdit {
        start: TextPoint::from_point(returns.start_position()),
        end: TextPoint::from_point(returns.end_position()),
        replacement: binding,
    });
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

fn first_annotation_removal(
    resolver: &Resolver<'_, '_>,
    function: Node<'_>,
    class_name: &str,
) -> Option<TextEdit> {
    let first = *Signature::of(function).positional.first()?;
    let annotation = unwrapped(annotation(first)?);
    let name = first
        .child_by_field_name(NAME_FIELD)
        .or_else(|| named(first).into_iter().next())?;
    is_class_reference(resolver, annotation, class_name).then(|| TextEdit {
        start: TextPoint::from_point(name.end_position()),
        end: TextPoint::from_point(annotation.end_position()),
        replacement: String::new(),
    })
}

// `Clase` o `type[Clase]` (también `typing.Type[Clase]`).
fn is_class_reference(resolver: &Resolver<'_, '_>, annotation: Node<'_>, class_name: &str) -> bool {
    let is_class_name =
        |node: Node<'_>| node.kind() == IDENTIFIER && text(node, resolver.source) == class_name;
    if is_class_name(annotation) {
        return true;
    }
    let Some((value, slices)) = subscript_parts(annotation) else {
        return false;
    };
    let is_type = is_builtin_named(resolver.model, value, resolver.source, TYPE_BUILTIN)
        || resolver
            .qualified(value)
            .is_some_and(|name| is_typing(&name, TYPING_TYPE));
    is_type && matches!(slices.as_slice(), [only] if is_class_name(unwrapped(*only)))
}

// El `might_be_generic` de ruff: la clase tiene parámetros de tipo, o alguna base es `X[...]` con algo
// que podría ser una variable de tipo.
fn might_be_generic(resolver: &Resolver<'_, '_>, class: Node<'_>) -> bool {
    class.child_by_field_name(TYPE_PARAMETERS_FIELD).is_some()
        || bases(class).into_iter().any(|base| {
            subscript_parts(base).is_some_and(|(_, slices)| {
                slices
                    .into_iter()
                    .any(|slice| might_be_type_variable(resolver, unwrapped(slice)))
            })
        })
}

// `AnyStr`, un atributo, `*X` o un nombre que no se sabe qué es o que está ligado una sola vez a
// `TypeVar(...)`, `ParamSpec(...)` o `TypeVarTuple(...)`.
fn might_be_type_variable(resolver: &Resolver<'_, '_>, expression: Node<'_>) -> bool {
    if resolver
        .qualified(expression)
        .is_some_and(|name| is_typing(&name, ANY_STR))
    {
        return true;
    }
    match expression.kind() {
        ATTRIBUTE => true,
        LIST_SPLAT => named(expression)
            .into_iter()
            .next()
            .is_some_and(|value| might_be_type_variable(resolver, unwrapped(value))),
        IDENTIFIER => might_be_old_style_type_variable(resolver, text(expression, resolver.source)),
        _ => false,
    }
}

fn might_be_old_style_type_variable(resolver: &Resolver<'_, '_>, name: &str) -> bool {
    let model = resolver.model;
    let Resolution::Binding(id) = model.lookup_from(resolver.scope, name) else {
        return !is_builtin(name);
    };
    let Some(binding) = model.binding(id) else {
        return false;
    };
    if model.shadowed_in_scope(binding.scope, id).is_some() {
        return !is_builtin(name);
    }
    binding.kind == BindingKind::Assignment
        && binding
            .site
            .node
            .filter(|target| {
                target
                    .parent()
                    .is_none_or(|assignment| assignment.child_by_field_name(TYPE_FIELD).is_none())
            })
            .and_then(assigned_call)
            .and_then(|function| resolver.qualified(function))
            .is_some_and(|qualified| {
                TYPE_VARIABLE_FACTORIES
                    .iter()
                    .any(|factory| is_typing(&qualified, factory))
            })
}
