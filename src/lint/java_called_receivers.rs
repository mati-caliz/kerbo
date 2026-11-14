use tree_sitter::Node;

use super::java_chains::argument_count;
use super::java_null_inference::{CallTarget, call_target};
use super::java_receiver_arguments::{ReceiverArguments, receiver_arguments};
use super::java_receivers::{Receiver, StaticImports, Variables};

const INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";

// El receptor que es otra llamada sobre un tipo conocido (`Splitter.on(x)` en
// `Splitter.on(x).split(y)`): el índice lo resuelve por lo que devuelve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalledReceiver {
    pub name: String,
    pub arity: usize,
    pub receiver: Receiver,
}

// Lo que se sabe del receptor de una llamada, también cuando es otra llamada.
pub(super) fn receiver_facts(
    call: Node<'_>,
    source: &[u8],
    imported: &StaticImports,
    variables: &Variables,
) -> ReceiverArguments {
    let mut facts = receiver_arguments(call, source);
    facts.called = call
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| call.kind() == INVOCATION && object.kind() == INVOCATION)
        .and_then(|object| {
            let CallTarget {
                name,
                is_constructor,
                receiver,
            } = call_target(object, source, imported, variables)?;
            (!is_constructor && matches!(receiver, Receiver::Type(_))).then(|| CalledReceiver {
                name,
                arity: argument_count(object),
                receiver,
            })
        })
        .map(Box::new);
    facts
}
