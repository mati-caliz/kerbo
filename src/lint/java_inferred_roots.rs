use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_called_receivers::receiver_facts;
use super::java_chains::{ChainRoot, argument_count, call_arguments, origin_chain};
use super::java_null_inference::{ArgumentShape, CallTarget, NullArgument, call_target};
use super::java_receivers::{StaticImports, Variables, inferred_parameters, simple_type_name};
use crate::syntax::{descendants, one_based};

const LAMBDA: &str = "lambda_expression";
const METHODS: &[&str] = &["method_declaration", "constructor_declaration"];
const LOCAL: &str = "local_variable_declaration";
const DECLARATOR: &str = "variable_declarator";
const INFERRED: &str = "var";
const TYPED_VALUES: &[&str] = &["object_creation_expression", "cast_expression"];
const INVOCATION: &str = "method_invocation";
const ARGUMENT_LIST: &str = "argument_list";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";

// De dónde sale el tipo de una variable que no lo tiene escrito: del parámetro de la interfaz
// funcional, si es el parámetro de una lambda pasada a una llamada (`sesion.then(r -> ...)`), o de
// lo que la inicializa, si es un `var` (`var agente = j.createSlave()`). El índice lo resuelve.
pub(super) fn inferred_root(usage: Node<'_>, source: &[u8], variables: &Variables) -> Option<ChainRoot> {
    let name = text(usage, source);
    let mut scope = usage.parent();
    while let Some(current) = scope {
        if current.kind() == LAMBDA
            && let Some(position) = inferred_parameters(current)
                .iter()
                .position(|parameter| text(*parameter, source) == name)
        {
            return lambda_parameter(current, position, source, variables);
        }
        if METHODS.contains(&current.kind()) {
            return initialized(current, usage, source, variables);
        }
        scope = current.parent();
    }
    None
}

fn lambda_parameter(
    lambda: Node<'_>,
    position: usize,
    source: &[u8],
    variables: &Variables,
) -> Option<ChainRoot> {
    let call = lambda
        .parent()
        .filter(|arguments| arguments.kind() == ARGUMENT_LIST)?
        .parent()?;
    let index = call_arguments(call)
        .iter()
        .position(|argument| argument.id() == lambda.id())?;
    let top_level = std::iter::successors(Some(call), Node::parent).last()?;
    let imported = StaticImports::of(top_level, source);
    let CallTarget {
        name,
        is_constructor,
        receiver,
    } = call_target(call, source, &imported, variables)?;
    let start = lambda.start_position();
    Some(ChainRoot::LambdaParameter {
        slot: Box::new(NullArgument {
            name,
            is_constructor,
            arity: argument_count(call),
            receiver,
            index,
            line: one_based(start.row),
            column: one_based(start.column),
            cast: None,
            exact: false,
            shape: ArgumentShape::MaybeArray,
            receiver_arguments: receiver_facts(call, source, &imported, variables),
        }),
        position,
    })
}

// El `var` tiene que estar declarado una sola vez en el método, antes del uso.
fn initialized(method: Node<'_>, usage: Node<'_>, source: &[u8], variables: &Variables) -> Option<ChainRoot> {
    let name = text(usage, source);
    let mut declarators = descendants(method).into_iter().filter(|node| {
        node.kind() == DECLARATOR
            && node
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    });
    let declarator = declarators.next()?;
    let written = declarator
        .parent()
        .filter(|declaration| declaration.kind() == LOCAL)?
        .child_by_field_name(TYPE_FIELD)?;
    if declarators.next().is_some()
        || text(written, source) != INFERRED
        || declarator.start_byte() >= usage.start_byte()
    {
        return None;
    }
    let value = declarator.child_by_field_name(VALUE_FIELD)?;
    if TYPED_VALUES.contains(&value.kind()) {
        return value
            .child_by_field_name(TYPE_FIELD)
            .map(|created| ChainRoot::Instance(simple_type_name(text(created, source))));
    }
    (value.kind() == INVOCATION)
        .then(|| origin_chain(value, source, variables))
        .flatten()
        .map(|chain| ChainRoot::Returned(Box::new(chain)))
}
