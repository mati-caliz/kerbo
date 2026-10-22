mod collections;
mod constants;
mod expressions;
mod receivers;
mod statements;
pub mod tables;
mod values;

use tree_sitter::Node;

pub use values::{Atom, Dangers, Flow};

use super::java::{NAME_FIELD, text};
use super::java_owner::{annotation_names, owner_of};
use super::java_receivers::{Receiver, StaticImports, Variables};
use crate::syntax::named_children;
use statements::Interpreter;
use values::{State, Value};

const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const VARIABLE_DECLARATOR: &str = "variable_declarator";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkKind {
    Sql,
    Command,
    Path,
    Html,
}

impl SinkKind {
    pub const ALL: [Self; 4] = [Self::Sql, Self::Command, Self::Path, Self::Html];
    pub const ALL_BITS: u8 = 0b1111;

    pub const fn index(self) -> usize {
        match self {
            Self::Sql => 0,
            Self::Command => 1,
            Self::Path => 2,
            Self::Html => 3,
        }
    }

    pub const fn bit(self) -> u8 {
        match self {
            Self::Sql => 0b0001,
            Self::Command => 0b0010,
            Self::Path => 0b0100,
            Self::Html => 0b1000,
        }
    }
}

// Sobre qué se llama un posible sumidero: un tipo conocido, un campo estático de otra clase (cuyo
// tipo sabe el índice) o un constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkReceiver {
    Type(String),
    StaticField { owner: String, field: String },
    Constructor(String),
}

// Una llamada que puede recibir una consulta, un comando o una ruta, con lo que lleva cada argumento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintSink {
    pub callee: String,
    pub receiver: SinkReceiver,
    pub arguments: Vec<Flow>,
    pub line: usize,
    pub column: usize,
}

// Una llamada cuyo resultado decide el índice: a un método del proyecto, lo que devuelve él; si
// no, lo que traen el receptor y los argumentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintCall {
    pub name: String,
    pub receiver: Receiver,
    pub receiver_flow: Flow,
    pub arguments: Vec<Flow>,
}

// Lo que un método hace con los datos: qué devuelve, qué llama y qué llega a cada sumidero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintMethod {
    pub name: String,
    pub owner: String,
    pub arity: usize,
    pub returns: Flow,
    pub calls: Vec<TaintCall>,
    pub sinks: Vec<TaintSink>,
}

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
