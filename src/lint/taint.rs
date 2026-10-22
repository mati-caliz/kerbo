pub mod folding;
mod values;

pub use values::{Atom, Dangers, Flow, Literal, State, Value, join_states};

use super::java_receivers::Receiver;

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
// tipo sabe el índice), un constructor, o un sumidero que el archivo ya reconoce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkReceiver {
    Type(String),
    StaticField { owner: String, field: String },
    Constructor(String),
    Known(SinkKind),
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
