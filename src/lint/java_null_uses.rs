use tree_sitter::Node;

use super::java::NAME_FIELD;
use super::java_called_receivers::receiver_facts;
use super::java_chains::argument_count;
use super::java_null_conditions::without_cast;
use super::java_null_flow::{
    ARGUMENTS_FIELD, Analysis, COMMENTS, Dereference, IDENTIFIER, INVOCATION, NullFlow, NullOrigin,
    OBJECT_FIELD, State,
};
use super::java_null_forwarding::forwarding_chain;
use super::java_null_inference::{CallTarget, NullArgument, call_target};
use super::java_null_keys::{forget, spellings, tracked_key};
use super::java_null_origin::ArgumentShape;
use super::java_receiver_arguments::ReceiverArguments;
use super::java_receivers::declared_type;
use crate::syntax::{named_children, one_based};

const FIELD_ACCESS: &str = "field_access";
const ARRAY_ACCESS: &str = "array_access";
const FIELD_FIELD: &str = "field";
const ARRAY_FIELD: &str = "array";
const CAST: &str = "cast_expression";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";
const INDEX_FIELD: &str = "index";
const INTEGER_LITERAL: &str = "decimal_integer_literal";
const ARRAY_TYPE: &str = "array_type";
const DIMENSIONS_FIELD: &str = "dimensions";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];

// Las variables posiblemente null que la llamada recibe como argumento. Siguen en el estado, en
// un flujo nuevo que recuerda la llamada: si el método no las desreferencia, el uso que viene
// después es el que cuenta.
pub(super) fn passed(analysis: &mut Analysis<'_, '_>, call: Node<'_>, mut state: State) -> State {
    let arguments: Vec<Node<'_>> = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .map(without_cast)
        .collect();
    let untracked = untracked_arguments(analysis, &arguments, &state);
    let passed: Vec<(usize, Node<'_>, String, usize)> = arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            let key = tracked_key(analysis, *argument)?;
            let origin = *state.get(&key)?;
            Some((index, *argument, key, origin))
        })
        .collect();
    if passed.is_empty() && untracked.is_empty() {
        return state;
    }
    let Some(target) = call_target(call, analysis.source, analysis.imported, analysis.variables) else {
        return state;
    };
    let call_use = CallUse {
        target,
        arity: argument_count(call),
        receiver_arguments: receiver_facts(call, analysis.source, analysis.imported, analysis.variables),
    };
    for (index, argument, origin) in untracked {
        let mut flow = NullFlow::new(origin);
        flow.arguments.push(call_use.at(index, argument));
        analysis.flows.push(flow);
    }
    for (index, argument, key, origin) in passed {
        let mut used = call_use.at(index, argument);
        if declares_single_value(analysis, argument, &key) {
            used.shape = ArgumentShape::Single;
        }
        let Some(flow) = analysis.flows.get_mut(origin) else {
            continue;
        };
        flow.arguments.push(used.clone());
        let mut passed_before = flow.passed_before.clone();
        passed_before.push(used);
        let after = NullFlow {
            passed_before,
            ..NullFlow::new(flow.origin.clone())
        };
        for spelling in spellings(analysis, key) {
            state.insert(spelling, analysis.flows.len());
        }
        analysis.flows.push(after);
    }
    state
}

// Los argumentos que pueden ser null sin que el flujo los siga: el elemento de un arreglo
// `@Nullable` y la llamada sin receptor que recibe algo posiblemente null (si el método sólo
// devuelve ese parámetro, se sabe en el índice).
fn untracked_arguments<'tree>(
    analysis: &Analysis<'_, '_>,
    arguments: &[Node<'tree>],
    state: &State,
) -> Vec<(usize, Node<'tree>, NullOrigin)> {
    arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            if is_nullable_element(analysis, *argument) {
                return Some((index, *argument, NullOrigin::Annotated));
            }
            if argument.kind() != INVOCATION {
                return None;
            }
            let chain = forwarding_chain(analysis, *argument, state)?;
            (!chain.forwarded.is_empty()).then_some((index, *argument, NullOrigin::Call(chain)))
        })
        .collect()
}

// La llamada que recibe los argumentos, con lo que hace falta para anotar cada uso.
struct CallUse {
    target: CallTarget,
    arity: usize,
    receiver_arguments: ReceiverArguments,
}

impl CallUse {
    fn at(&self, index: usize, argument: Node<'_>) -> NullArgument {
        let start = argument.start_position();
        NullArgument {
            name: self.target.name.clone(),
            is_constructor: self.target.is_constructor,
            arity: self.arity,
            receiver: self.target.receiver.clone(),
            index,
            line: one_based(start.row),
            column: one_based(start.column),
            cast: None,
            exact: false,
            shape: ArgumentShape::MaybeArray,
            receiver_arguments: self.receiver_arguments.clone(),
        }
    }
}

// IntelliJ marca la desreferencia de un campo en el calificador: en `this.x.y`, en `x`.
fn qualifier_name(object: Node<'_>) -> Node<'_> {
    object
        .child_by_field_name(FIELD_FIELD)
        .filter(|_| object.kind() == FIELD_ACCESS)
        .unwrap_or(object)
}

pub(super) fn dereference(analysis: &mut Analysis<'_, '_>, node: Node<'_>, mut state: State) -> State {
    let (receiver, used) = match node.kind() {
        INVOCATION => (
            node.child_by_field_name(OBJECT_FIELD),
            node.child_by_field_name(NAME_FIELD),
        ),
        FIELD_ACCESS => {
            let object = node.child_by_field_name(OBJECT_FIELD);
            (object, object.map(qualifier_name))
        }
        ARRAY_ACCESS => (
            node.child_by_field_name(ARRAY_FIELD),
            node.child_by_field_name(ARRAY_FIELD),
        ),
        CAST if casts_to_primitive(node) => (
            node.child_by_field_name(VALUE_FIELD),
            node.child_by_field_name(VALUE_FIELD),
        ),
        _ => (None, None),
    };
    let receiver = receiver.map(without_cast);
    if let (Some(element), Some(used)) = (receiver, used)
        && is_nullable_element(analysis, element)
    {
        let mut flow = NullFlow::new(NullOrigin::Annotated);
        flow.dereferences.push(dereference_at(analysis, used));
        analysis.flows.push(flow);
        return state;
    }
    let (Some(receiver), Some(used)) = (
        receiver.and_then(|receiver| tracked_key(analysis, receiver)),
        used,
    ) else {
        return state;
    };
    if let Some(origin) = forget(&mut state, &receiver) {
        let used = dereference_at(analysis, used);
        if let Some(flow) = analysis.flows.get_mut(origin) {
            flow.dereferences.push(used);
        }
    }
    state
}

fn dereference_at(analysis: &Analysis<'_, '_>, used: Node<'_>) -> Dereference {
    let start = used.start_position();
    Dereference {
        name: analysis.text(used).to_owned(),
        line: one_based(start.row),
        column: one_based(start.column),
    }
}

// Una variable declarada con un tipo que no es arreglo (tampoco `String nombres[]`): en un varargs
// va siempre como elemento.
fn declares_single_value(analysis: &Analysis<'_, '_>, argument: Node<'_>, key: &str) -> bool {
    argument.kind() == IDENTIFIER
        && declared_type(argument, key, analysis.source).is_some_and(|written| {
            written.kind() != ARRAY_TYPE && !declares_dimensions(analysis, written, key)
        })
}

fn declares_dimensions(analysis: &Analysis<'_, '_>, written: Node<'_>, key: &str) -> bool {
    written.parent().is_some_and(|declaration| {
        declaration.child_by_field_name(DIMENSIONS_FIELD).is_some()
            || named_children(declaration).into_iter().any(|declarator| {
                declarator.kind() == VARIABLE_DECLARATOR
                    && declarator
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|name| analysis.text(name) == key)
                    && declarator.child_by_field_name(DIMENSIONS_FIELD).is_some()
            })
    })
}

// La lectura de un elemento de un parámetro arreglo `@Nullable` con un índice que no es constante:
// IntelliJ no sigue esos elementos, así que cada lectura puede ser null aunque antes se haya
// chequeado o asignado.
pub(super) fn is_nullable_element(analysis: &Analysis<'_, '_>, node: Node<'_>) -> bool {
    node.kind() == ARRAY_ACCESS
        && node
            .child_by_field_name(INDEX_FIELD)
            .is_some_and(|index| index.kind() != INTEGER_LITERAL)
        && node
            .child_by_field_name(ARRAY_FIELD)
            .filter(|array| array.kind() == IDENTIFIER)
            .is_some_and(|array| {
                analysis
                    .nullable_elements
                    .iter()
                    .any(|name| name == analysis.text(array))
            })
}

// `(int) valor` desempaqueta el valor: si es null, falla como una desreferencia.
fn casts_to_primitive(cast: Node<'_>) -> bool {
    cast.child_by_field_name(TYPE_FIELD)
        .is_some_and(|written| PRIMITIVE_TYPES.contains(&written.kind()))
}
