use tree_sitter::Node;

use super::model::{Model, ScopeKind};
use super::qualified::is_builtin_named;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, CLASS, COMMENT, DECORATED, FUNCTION, FUNCTION_FIELD,
    IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, NAME_FIELD, OBJECT_FIELD, TRUE, VALUE_FIELD, named, text,
};
use crate::lint::python::semantic::qualified::qualified_name;

const SUPER: &str = "super";
const CLASS_CELL: &str = "__class__";
const PARAMETERS_FIELD: &str = "parameters";
const DECORATOR: &str = "decorator";
const POSITIONAL_SEPARATOR: &str = "positional_separator";
const NAMED_PARAMETERS: [&str; 2] = ["typed_parameter", "identifier"];
const DEFAULTED_PARAMETERS: [&str; 2] = ["default_parameter", "typed_default_parameter"];
const DATACLASS: &str = "dataclasses.dataclass";
const SLOTS: &str = "slots";

// UP008: `super(Clase, self)` dentro de un método de `Clase`, donde alcanza con `super()`.
pub fn is_super_with_parameters(model: &Model<'_>, source: &[u8], call: Node<'_>) -> bool {
    let is_super = call.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
        function.kind() == IDENTIFIER && is_builtin_named(model, function, source, SUPER)
    });
    let scope = model.current();
    let callable_kind = match scope.kind {
        ScopeKind::Function => FUNCTION,
        ScopeKind::Lambda => LAMBDA,
        ScopeKind::Module | ScopeKind::Class | ScopeKind::Generator | ScopeKind::Type => return false,
    };
    let positional: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !matches!(argument.kind(), COMMENT | KEYWORD_ARGUMENT))
        .collect();
    let [class, instance] = positional.as_slice() else {
        return false;
    };
    let Some(callable) = ancestors(call).find(|ancestor| ancestor.kind() == callable_kind) else {
        return false;
    };
    let receiver = first_parameter(callable).map(|parameter| text(parameter, source));
    let names_receiver = instance.kind() == IDENTIFIER && receiver == Some(text(*instance, source));
    let classes: Vec<Node<'_>> = ancestors(callable)
        .filter(|ancestor| ancestor.kind() == CLASS)
        .collect();
    let Some(innermost) = classes.first() else {
        return false;
    };
    let locally_bound = class.kind() == IDENTIFIER && scope.names.contains_key(text(*class, source));
    is_super
        && names_receiver
        && !locally_bound
        && names_enclosing_class(*class, &classes, source)
        && !has_slotted_dataclass(model, source, *innermost)
}

fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

// El primer parámetro posicional común, como `args` en el AST de ruff (sin los anteriores a `/`).
fn first_parameter(callable: Node<'_>) -> Option<Node<'_>> {
    let parameters = named(callable.child_by_field_name(PARAMETERS_FIELD)?);
    let after_separator = parameters
        .iter()
        .position(|parameter| parameter.kind() == POSITIONAL_SEPARATOR)
        .map_or(0, |separator| separator.saturating_add(1));
    let first = parameters.get(after_separator)?;
    if NAMED_PARAMETERS.contains(&first.kind()) {
        return if first.kind() == IDENTIFIER {
            Some(*first)
        } else {
            named(*first).first().copied()
        };
    }
    DEFAULTED_PARAMETERS
        .contains(&first.kind())
        .then(|| first.child_by_field_name(NAME_FIELD))
        .flatten()
}

fn names_enclosing_class(class: Node<'_>, classes: &[Node<'_>], source: &[u8]) -> bool {
    let chain = attribute_chain(class, source);
    if chain == [CLASS_CELL] {
        return true;
    }
    let names: Vec<&str> = classes
        .iter()
        .filter_map(|enclosing| enclosing.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source))
        .collect();
    !chain.is_empty() && chain.len() == names.len() && chain.iter().rev().eq(names.iter())
}

fn attribute_chain<'source>(expression: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    match expression.kind() {
        IDENTIFIER => vec![text(expression, source)],
        ATTRIBUTE => {
            let (Some(object), Some(attribute)) = (
                expression.child_by_field_name(OBJECT_FIELD),
                expression.child_by_field_name(ATTRIBUTE_FIELD),
            ) else {
                return Vec::new();
            };
            let mut chain = attribute_chain(object, source);
            if !chain.is_empty() {
                chain.push(text(attribute, source));
            }
            chain
        }
        _ => Vec::new(),
    }
}

// `@dataclass(slots=True)` arma una clase nueva, y `super()` sin argumentos apunta a la vieja.
fn has_slotted_dataclass(model: &Model<'_>, source: &[u8], class: Node<'_>) -> bool {
    let Some(decorated) = class.parent().filter(|parent| parent.kind() == DECORATED) else {
        return false;
    };
    named(decorated)
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .filter_map(|decorator| named(decorator).first().copied())
        .filter(|expression| expression.kind() == CALL)
        .filter(|call| {
            call.child_by_field_name(FUNCTION_FIELD)
                .and_then(|function| qualified_name(model, function, source))
                .is_some_and(|name| name == DATACLASS)
        })
        .filter_map(|call| call.child_by_field_name(ARGUMENTS_FIELD))
        .flat_map(named)
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .any(|argument| {
            argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == SLOTS)
                && argument
                    .child_by_field_name(VALUE_FIELD)
                    .is_some_and(|value| value.kind() == TRUE)
        })
}
