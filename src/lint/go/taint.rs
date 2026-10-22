mod calls;
mod expressions;
mod imports;
mod statements;
mod tables;
mod types;

use tree_sitter::Node;

use crate::lint::taint::{Atom, Flow, State, TaintMethod, Value};
use crate::syntax::descendants;
use imports::Imports;
use statements::Interpreter;
use types::Types;

const FUNCTION: &str = "function_declaration";
const METHOD: &str = "method_declaration";
const FUNCTION_LITERAL: &str = "func_literal";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const PARAMETERS_FIELD: &str = "parameters";
const RECEIVER_FIELD: &str = "receiver";
const QUALIFIER_SEPARATOR: &str = ".";
const ANONYMOUS_PREFIX: &str = "<anónima>:";

// Lo que cada función del archivo hace con los datos, para cruzarlo en el índice. Las anónimas (el
// `func(w, r)` de un `HandleFunc`) también, con su línea como nombre; ven los tipos de las
// variables de las funciones que las contienen.
pub fn go_taints(root: Node<'_>, source: &[u8]) -> Vec<TaintMethod> {
    let imports = Imports::of(root, source);
    descendants(root)
        .into_iter()
        .filter(|node| matches!(node.kind(), FUNCTION | METHOD | FUNCTION_LITERAL))
        .filter_map(|function| summary(function, root, source, &imports))
        .collect()
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    node.utf8_text(source).unwrap_or_default()
}

// La función de más afuera que contiene a esta: sus variables se ven desde las anónimas de adentro.
fn outermost(function: Node<'_>) -> Node<'_> {
    let mut current = function;
    let mut cursor = function.parent();
    while let Some(parent) = cursor {
        if matches!(parent.kind(), FUNCTION | METHOD | FUNCTION_LITERAL) {
            current = parent;
        }
        cursor = parent.parent();
    }
    current
}

// `Handler`, `Store.Get` para un método, o la línea de una anónima.
fn function_name(function: Node<'_>, source: &[u8], imports: &Imports) -> String {
    let Some(name) = function.child_by_field_name(NAME_FIELD) else {
        return format!("{ANONYMOUS_PREFIX}{}", function.start_position().row);
    };
    let owner = function
        .child_by_field_name(RECEIVER_FIELD)
        .and_then(|receiver| types::parameters(receiver).into_iter().next())
        .and_then(|(_, typ)| typ)
        .and_then(|typ| types::type_name(typ, source, imports))
        .and_then(|written| written.strip_prefix(types::LOCAL_PREFIX).map(str::to_owned));
    let own = text(name, source);
    owner.map_or_else(
        || own.to_owned(),
        |owner| format!("{owner}{QUALIFIER_SEPARATOR}{own}"),
    )
}

fn summary(function: Node<'_>, root: Node<'_>, source: &[u8], imports: &Imports) -> Option<TaintMethod> {
    let body = function.child_by_field_name(BODY_FIELD)?;
    let types = Types::of(outermost(function), root, source, imports);
    let parameters = function
        .child_by_field_name(PARAMETERS_FIELD)
        .map(types::parameters)
        .unwrap_or_default();
    let mut state = State::new();
    for (position, (name, typ)) in parameters.iter().enumerate() {
        let from_request = typ
            .and_then(|typ| types::type_name(typ, source, imports))
            .is_some_and(|written| tables::is_request_type(&written));
        let atom = if from_request {
            Atom::Source
        } else {
            Atom::Parameter(position)
        };
        if let Some(name) = name {
            state.insert(text(*name, source).to_owned(), Value::Data(Flow::of(atom)));
        }
    }
    let mut interpreter = Interpreter::new(source, imports, types);
    interpreter.run(body, state);
    Some(TaintMethod {
        name: function_name(function, source, imports),
        owner: String::new(),
        arity: parameters.len(),
        returns: interpreter.returns,
        calls: interpreter.calls,
        sinks: interpreter.sinks,
    })
}
