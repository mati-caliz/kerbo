use std::collections::HashMap;

use super::SinkKind;

// De dónde puede venir un dato: lo que manda el usuario, un parámetro del método o lo que
// devuelve una llamada (que decide el índice, porque puede ser de otro archivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Atom {
    Source,
    Parameter(usize),
    Call(usize),
}

// Para qué sumideros sigue siendo peligroso un dato, un bit por tipo: un escape de HTML le saca el
// de HTML, un chequeo de `../` el de las rutas, y lo que queda sirve para lo demás.
pub type Dangers = u8;

pub const ALL_DANGERS: Dangers = SinkKind::ALL_BITS;

// Lo que puede llevar un valor: de dónde vienen sus datos y para qué siguen siendo peligrosos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flow {
    pub atoms: Vec<(Atom, Dangers)>,
}

impl Flow {
    pub fn of(atom: Atom) -> Self {
        Self {
            atoms: vec![(atom, ALL_DANGERS)],
        }
    }

    pub fn absorb(&mut self, other: &Self) {
        for (atom, dangers) in &other.atoms {
            match self.atoms.binary_search_by(|(existing, _)| existing.cmp(atom)) {
                Ok(position) => self.merge_at(position, *dangers),
                Err(position) => self.atoms.insert(position, (*atom, *dangers)),
            }
        }
    }

    fn merge_at(&mut self, position: usize, dangers: Dangers) {
        if let Some((_, existing)) = self.atoms.get_mut(position) {
            *existing |= dangers;
        }
    }

    // El mismo valor después de algo que lo vuelve seguro para esos sumideros.
    pub fn without(&self, removed: Dangers) -> Self {
        Self {
            atoms: self
                .atoms
                .iter()
                .map(|(atom, dangers)| (*atom, dangers & !removed))
                .filter(|(_, dangers)| *dangers != 0)
                .collect(),
        }
    }

    pub fn escaped_for_html(&self) -> Self {
        self.without(SinkKind::Html.bit())
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

// `coleccion[indice]` y `objeto.propiedad`: con la posición o la clave constante, lo que tiene; si no,
// todo junto.
pub fn element(collection: &Value, index: &Value) -> Value {
    match (collection, index.literal()) {
        (Value::List(items), Some(Literal::Integer(position))) => usize::try_from(*position)
            .ok()
            .and_then(|position| items.get(position))
            .cloned()
            .unwrap_or_else(Value::clean),
        (Value::Map(entries), Some(Literal::Text(key)))
            if entries.iter().all(|(name, _)| !name.is_empty()) =>
        {
            entries
                .iter()
                .find(|(name, _)| name == key)
                .map_or_else(Value::clean, |(_, value)| value.clone())
        }
        (Value::Constant(_), _) => Value::clean(),
        _ => Value::Data(collection.flow()),
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
