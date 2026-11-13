use tree_sitter::Node;

use super::java::text;
use super::java_owner::{NullMarking, null_marking_of};
use super::java_receivers::declared_type;
use super::java_type_variables::is_type_parameter_in_scope;
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";
const IDENTIFIER: &str = "identifier";
const TYPE_ARGUMENTS: &str = "type_arguments";
const ANNOTATED_TYPE: &str = "annotated_type";
const WILDCARD: &str = "wildcard";
const TYPE_IDENTIFIER: &str = "type_identifier";
const NULLABLE: &str = "Nullable";

// Los argumentos de tipo con que se declara el receptor de una llamada (`Multiset<String> ms` en
// `ms.setCount(null, 1, 0)`) y lo que dice de nulos el código que llama. Bajo `@NullMarked`, un
// argumento concreto sin anotar no admite null, e IntelliJ lo pone en lugar de la variable de tipo
// del parámetro.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiverArguments {
    pub marking: NullMarking,
    pub arguments: Vec<ArgumentNullness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentNullness {
    Concrete,
    Nullable,
    Unknown,
}

pub fn receiver_arguments(call: Node<'_>, source: &[u8]) -> ReceiverArguments {
    let arguments = call
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| call.kind() == INVOCATION && object.kind() == IDENTIFIER)
        .and_then(|object| declared_type(object, text(object, source), source))
        .map(|written| arguments_of(call, written, source))
        .unwrap_or_default();
    if arguments.is_empty() {
        return ReceiverArguments::default();
    }
    ReceiverArguments {
        marking: null_marking_of(call, source),
        arguments,
    }
}

fn arguments_of(call: Node<'_>, written: Node<'_>, source: &[u8]) -> Vec<ArgumentNullness> {
    named_children(written)
        .into_iter()
        .filter(|child| child.kind() == TYPE_ARGUMENTS)
        .flat_map(named_children)
        .map(|argument| match argument.kind() {
            ANNOTATED_TYPE if text(argument, source).contains(NULLABLE) => ArgumentNullness::Nullable,
            ANNOTATED_TYPE | WILDCARD => ArgumentNullness::Unknown,
            TYPE_IDENTIFIER if is_type_parameter_in_scope(call, text(argument, source), source) => {
                ArgumentNullness::Unknown
            }
            _ => ArgumentNullness::Concrete,
        })
        .collect()
}
