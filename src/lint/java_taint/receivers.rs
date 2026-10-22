use tree_sitter::Node;

use super::statements::Interpreter;
use super::tables;
use super::values::Flow;
use super::{SinkReceiver, TaintSink};
use crate::lint::java::NAME_FIELD;
use crate::lint::java_receivers::simple_type_name;
use crate::syntax::one_based;

const IDENTIFIER: &str = "identifier";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const FIELD_ACCESS: &str = "field_access";
const THIS: &str = "this";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const TYPE_FIELD: &str = "type";
const QUALIFIER_SEPARATOR: char = '.';

fn starts_uppercase(name: &str) -> bool {
    name.starts_with(|first: char| first.is_ascii_uppercase())
}

pub(super) fn declared_type(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> Option<String> {
    let variable = match object.kind() {
        IDENTIFIER => object,
        FIELD_ACCESS
            if object
                .child_by_field_name(OBJECT_FIELD)
                .is_some_and(|owner| owner.kind() == THIS) =>
        {
            object.child_by_field_name(FIELD_FIELD)?
        }
        _ => return None,
    };
    interpreter
        .variables
        .get(interpreter.text(variable))
        .cloned()
        .flatten()
}

// Sobre qué tipo se llama un posible sumidero, si se sabe sin compilar.
pub(super) fn sink_receiver(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> Option<SinkReceiver> {
    if let Some(made) = (object.kind() == IDENTIFIER)
        .then(|| interpreter.factory_variables.get(interpreter.text(object)))
        .flatten()
    {
        return Some(SinkReceiver::Type(made.clone()));
    }
    if let Some(declared) = declared_type(interpreter, object) {
        return Some(SinkReceiver::Type(declared));
    }
    match object.kind() {
        IDENTIFIER => {
            let name = interpreter.text(object);
            (!interpreter.variables.contains_key(name) && starts_uppercase(name))
                .then(|| SinkReceiver::Type(name.to_owned()))
        }
        FIELD_ACCESS => {
            let segments: Vec<&str> = interpreter
                .text(object)
                .split(QUALIFIER_SEPARATOR)
                .map(str::trim)
                .collect();
            match segments.as_slice() {
                [.., owner, field] if starts_uppercase(owner) => Some(SinkReceiver::StaticField {
                    owner: (*owner).to_owned(),
                    field: (*field).to_owned(),
                }),
                [.., last] if starts_uppercase(last) => Some(SinkReceiver::Type((*last).to_owned())),
                _ => None,
            }
        }
        INVOCATION => {
            let owner = object.child_by_field_name(OBJECT_FIELD)?;
            let method = object.child_by_field_name(NAME_FIELD)?;
            let owner_type = declared_type(interpreter, owner)
                .unwrap_or_else(|| simple_type_name(interpreter.text(owner)));
            tables::factory_type(&owner_type, interpreter.text(method))
                .map(|known| SinkReceiver::Type(known.to_owned()))
        }
        CREATION => object
            .child_by_field_name(TYPE_FIELD)
            .map(|written| SinkReceiver::Type(simple_type_name(interpreter.text(written)))),
        _ => None,
    }
}

// La variable donde empieza una cadena de llamadas que guardan sus argumentos
// (`builder.append(a).append(b)` guarda todo en `builder`).
pub(super) fn chain_root(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> Option<String> {
    let mut current = object;
    while current.kind() == INVOCATION {
        let name = current.child_by_field_name(NAME_FIELD)?;
        if !tables::absorbs_arguments(interpreter.text(name)) {
            return None;
        }
        current = current.child_by_field_name(OBJECT_FIELD)?;
    }
    (current.kind() == IDENTIFIER).then(|| interpreter.text(current).to_owned())
}

pub(super) fn sink_at(
    node: Node<'_>,
    callee: &str,
    receiver: SinkReceiver,
    arguments: Vec<Flow>,
) -> TaintSink {
    let start = node
        .child_by_field_name(NAME_FIELD)
        .or_else(|| node.child_by_field_name(TYPE_FIELD))
        .unwrap_or(node)
        .start_position();
    TaintSink {
        callee: callee.to_owned(),
        receiver,
        arguments,
        line: one_based(start.row),
        column: one_based(start.column),
    }
}
