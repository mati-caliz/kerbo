use std::collections::HashMap;

// De dónde puede venir un dato: lo que manda el usuario, un parámetro del método o lo que
// devuelve una llamada (que decide el índice, porque puede ser de otro archivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Atom {
    Source,
    Parameter(usize),
    Call(usize),
}

// Lo que puede llevar un valor, separado en lo que llega tal cual y lo que pasó por un escape de
// HTML: escapado no sirve para un XSS, pero sigue armando una consulta o una ruta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flow {
    pub atoms: Vec<Atom>,
    pub escaped: Vec<Atom>,
}

fn insert_sorted(atoms: &mut Vec<Atom>, atom: Atom) {
    if let Err(position) = atoms.binary_search(&atom) {
        atoms.insert(position, atom);
    }
}

impl Flow {
    pub fn of(atom: Atom) -> Self {
        Self {
            atoms: vec![atom],
            escaped: Vec::new(),
        }
    }

    pub fn absorb(&mut self, other: &Self) {
        for atom in &other.atoms {
            insert_sorted(&mut self.atoms, *atom);
        }
        for atom in &other.escaped {
            insert_sorted(&mut self.escaped, *atom);
        }
    }

    // El mismo valor después de un escape de HTML.
    pub fn escaped_for_html(&self) -> Self {
        let mut escaped = self.escaped.clone();
        for atom in &self.atoms {
            insert_sorted(&mut escaped, *atom);
        }
        Self {
            atoms: Vec::new(),
            escaped,
        }
    }

    pub fn union(mut self, other: &Self) -> Self {
        self.absorb(other);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Text(String),
    Integer(i64),
    Character(char),
    Boolean(bool),
}

// Lo que se sabe de un valor: una constante, de dónde pueden venir sus datos, o una lista o un
// mapa armados en el método, con lo que tiene cada posición o clave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Constant(Literal),
    Data(Flow),
    List(Vec<Self>),
    Map(Vec<(String, Self)>),
}

impl Value {
    pub fn clean() -> Self {
        Self::Data(Flow::default())
    }

    pub fn flow(&self) -> Flow {
        match self {
            Self::Constant(_) => Flow::default(),
            Self::Data(flow) => flow.clone(),
            Self::List(items) => items
                .iter()
                .fold(Flow::default(), |flow, item| flow.union(&item.flow())),
            Self::Map(entries) => entries
                .iter()
                .fold(Flow::default(), |flow, (_, item)| flow.union(&item.flow())),
        }
    }

    pub const fn literal(&self) -> Option<&Literal> {
        match self {
            Self::Constant(literal) => Some(literal),
            Self::Data(_) | Self::List(_) | Self::Map(_) => None,
        }
    }

    // Lo que puede valer después de dos caminos: lo mismo si coinciden, la misma forma si las
    // dos son listas o mapas iguales en forma, y si no, los datos de las dos.
    pub fn join(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        match (self, other) {
            (Self::List(first), Self::List(second)) if first.len() == second.len() => Self::List(
                first
                    .into_iter()
                    .zip(second)
                    .map(|(one, two)| one.join(two))
                    .collect(),
            ),
            (Self::Map(first), Self::Map(second))
                if first.len() == second.len()
                    && first.iter().zip(&second).all(|((one, _), (two, _))| one == two) =>
            {
                Self::Map(
                    first
                        .into_iter()
                        .zip(second)
                        .map(|((key, one), (_, two))| (key, one.join(two)))
                        .collect(),
                )
            }
            (one, two) => Self::Data(one.flow().union(&two.flow())),
        }
    }
}

pub type State = HashMap<String, Value>;

pub fn join_states(first: Option<State>, second: Option<State>) -> Option<State> {
    match (first, second) {
        (Some(mut merged), Some(other)) => {
            for (name, value) in other {
                let joined = match merged.remove(&name) {
                    Some(existing) => existing.join(value),
                    None => value,
                };
                merged.insert(name, joined);
            }
            Some(merged)
        }
        (one, None) | (None, one) => one,
    }
}
