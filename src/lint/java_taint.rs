mod collections;
mod constants;
mod expressions;
mod receivers;
mod statements;
pub mod tables;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_owner::{annotation_names, owner_of};
use super::java_receivers::{StaticImports, Variables};
use crate::lint::taint::{Atom, Flow, State, TaintMethod, Value};
use crate::syntax::named_children;
use statements::Interpreter;

const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const VARIABLE_DECLARATOR: &str = "variable_declarator";

pub fn taint_method(
    declaration: Node<'_>,
    source: &[u8],
    variables: &Variables,
    imported: &StaticImports,
) -> Option<TaintMethod> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    let name = text(declaration.child_by_field_name(NAME_FIELD)?, source).to_owned();
    let parameters = declaration
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| parameter.kind() == FORMAL_PARAMETER || parameter.kind() == SPREAD_PARAMETER)
        .collect::<Vec<_>>();
    let mut state = State::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let Some(parameter_name) = parameter_name(*parameter) else {
            continue;
        };
        let value = if tables::is_source_parameter(&annotation_names(*parameter, source)) {
            Value::Data(Flow::of(Atom::Source))
        } else {
            Value::Data(Flow::of(Atom::Parameter(position)))
        };
        state.insert(text(parameter_name, source).to_owned(), value);
    }
    let mut interpreter = Interpreter::new(source, variables, imported);
    interpreter.run(body, state);
    Some(TaintMethod {
        name,
        owner: owner_of(declaration, source).name,
        arity: parameters.len(),
        returns: interpreter.returns,
        calls: interpreter.calls,
        sinks: interpreter.sinks,
    })
}

fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    if parameter.kind() == SPREAD_PARAMETER {
        return named_children(parameter)
            .into_iter()
            .find(|child| child.kind() == VARIABLE_DECLARATOR)
            .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD));
    }
    parameter.child_by_field_name(NAME_FIELD)
}
