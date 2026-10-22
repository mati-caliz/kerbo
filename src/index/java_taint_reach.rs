use crate::lint::java_taint::{Atom, Flow};

// Los parámetros, como bits: los de más allá del 64 no se siguen.
const TRACKED_PARAMETERS: usize = 64;

pub fn parameter_bit(position: usize) -> u64 {
    u32::try_from(position)
        .ok()
        .and_then(|shift| 1_u64.checked_shl(shift))
        .unwrap_or_default()
}

// De dónde puede venir un dato: del usuario, o de ciertos parámetros del método.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Origins {
    pub source: bool,
    pub parameters: u64,
}

impl Origins {
    const fn absorb(&mut self, other: Self) {
        self.source |= other.source;
        self.parameters |= other.parameters;
    }

    pub fn positions(self) -> impl Iterator<Item = usize> {
        (0..TRACKED_PARAMETERS).filter(move |position| self.parameters & parameter_bit(*position) != 0)
    }
}

// Qué puede llevar un valor, tal cual o después de un escape de HTML.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reach {
    pub raw: Origins,
    pub escaped: Origins,
}

// Qué parámetros de un método reciben algo del usuario desde alguna llamada, tal cual o escapado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tainted {
    pub raw: u64,
    pub escaped: u64,
}

impl Tainted {
    // Suma un parámetro que recibe algo del usuario; dice si cambió algo.
    pub fn mark(&mut self, position: usize, raw: bool) -> bool {
        let before = *self;
        if raw {
            self.raw |= parameter_bit(position);
        } else {
            self.escaped |= parameter_bit(position);
        }
        *self != before
    }
}

impl Reach {
    pub const fn absorb(&mut self, other: Self) {
        self.raw.absorb(other.raw);
        self.escaped.absorb(other.escaped);
    }

    pub const fn absorb_escaped(&mut self, other: Self) {
        self.escaped.absorb(other.raw);
        self.escaped.absorb(other.escaped);
    }

    // Si llega algo del usuario sin escapar: lo que importa para un XSS.
    pub const fn raw_user(self, tainted: Tainted) -> bool {
        self.raw.source || self.raw.parameters & tainted.raw != 0
    }

    // Si llega algo del usuario, escapado o no: lo que importa para una consulta, un comando o una ruta.
    pub const fn any_user(self, tainted: Tainted) -> bool {
        self.raw_user(tainted)
            || self.escaped.source
            || self.raw.parameters & tainted.escaped != 0
            || self.escaped.parameters & (tainted.raw | tainted.escaped) != 0
    }
}

pub fn evaluate(flow: &Flow, values: &[Reach]) -> Reach {
    let mut reach = Reach::default();
    for atom in &flow.atoms {
        match atom {
            Atom::Source => reach.raw.source = true,
            Atom::Parameter(position) => reach.raw.parameters |= parameter_bit(*position),
            Atom::Call(index) => {
                if let Some(value) = values.get(*index) {
                    reach.absorb(*value);
                }
            }
        }
    }
    for atom in &flow.escaped {
        match atom {
            Atom::Source => reach.escaped.source = true,
            Atom::Parameter(position) => reach.escaped.parameters |= parameter_bit(*position),
            Atom::Call(index) => {
                if let Some(value) = values.get(*index) {
                    reach.absorb_escaped(*value);
                }
            }
        }
    }
    reach
}

// Lo que devuelve una llamada a un método del proyecto: lo que él devuelve, con cada parámetro
// reemplazado por lo que trae el argumento.
pub fn substitute(returned: Reach, arguments: &[Reach]) -> Reach {
    let mut result = Reach::default();
    result.raw.source = returned.raw.source;
    result.escaped.source = returned.escaped.source;
    for argument in returned
        .raw
        .positions()
        .filter_map(|position| arguments.get(position))
    {
        result.absorb(*argument);
    }
    for argument in returned
        .escaped
        .positions()
        .filter_map(|position| arguments.get(position))
    {
        result.absorb_escaped(*argument);
    }
    result
}
