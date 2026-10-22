use crate::lint::java_taint::{Atom, Flow, SinkKind};

// Los parámetros, como bits: los de más allá del 64 no se siguen.
const TRACKED_PARAMETERS: usize = 64;
const KINDS: usize = SinkKind::ALL.len();

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

// Qué puede llevar un valor que siga siendo peligroso, por cada tipo de sumidero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reach {
    kinds: [Origins; KINDS],
}

// Qué parámetros de un método reciben desde alguna llamada algo del usuario peligroso para cada
// tipo de sumidero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tainted {
    kinds: [u64; KINDS],
}

fn kinds_in(dangers: u8) -> impl Iterator<Item = usize> {
    SinkKind::ALL
        .into_iter()
        .filter(move |kind| dangers & kind.bit() != 0)
        .map(SinkKind::index)
}

impl Tainted {
    // Suma un parámetro que recibe algo peligroso para esos sumideros; dice si cambió algo.
    pub fn mark(&mut self, position: usize, dangers: u8) -> bool {
        let before = *self;
        for kind in kinds_in(dangers) {
            if let Some(bits) = self.kinds.get_mut(kind) {
                *bits |= parameter_bit(position);
            }
        }
        *self != before
    }
}

impl Reach {
    pub fn absorb(&mut self, other: Self) {
        for (mine, theirs) in self.kinds.iter_mut().zip(other.kinds) {
            mine.absorb(theirs);
        }
    }

    fn origins_for(&mut self, dangers: u8) -> impl Iterator<Item = &mut Origins> {
        self.kinds
            .iter_mut()
            .zip(SinkKind::ALL)
            .filter(move |(_, kind)| dangers & kind.bit() != 0)
            .map(|(origins, _)| origins)
    }

    fn absorb_for(&mut self, other: Self, dangers: u8) {
        for kind in kinds_in(dangers) {
            if let (Some(mine), Some(theirs)) = (self.kinds.get_mut(kind), other.kinds.get(kind)) {
                mine.absorb(*theirs);
            }
        }
    }

    // Si llega algo del usuario que sigue siendo peligroso para ese sumidero.
    pub fn reaches(self, kind: SinkKind, tainted: Tainted) -> bool {
        let origins = self.kinds.get(kind.index()).copied().unwrap_or_default();
        let parameters = tainted.kinds.get(kind.index()).copied().unwrap_or_default();
        origins.source || origins.parameters & parameters != 0
    }

    // Para qué sumideros es peligroso lo que llega del usuario.
    pub fn dangers(self, tainted: Tainted) -> u8 {
        SinkKind::ALL
            .into_iter()
            .filter(|kind| self.reaches(*kind, tainted))
            .fold(0, |dangers, kind| dangers | kind.bit())
    }
}

pub fn evaluate(flow: &Flow, values: &[Reach]) -> Reach {
    let mut reach = Reach::default();
    for (atom, dangers) in &flow.atoms {
        match atom {
            Atom::Source => {
                for origins in reach.origins_for(*dangers) {
                    origins.source = true;
                }
            }
            Atom::Parameter(position) => {
                for origins in reach.origins_for(*dangers) {
                    origins.parameters |= parameter_bit(*position);
                }
            }
            Atom::Call(index) => {
                if let Some(value) = values.get(*index) {
                    reach.absorb_for(*value, *dangers);
                }
            }
        }
    }
    reach
}

// Lo que devuelve una llamada a un método del proyecto: lo que él devuelve, con cada parámetro
// reemplazado por lo que trae el argumento, tipo por tipo.
pub fn substitute(returned: Reach, arguments: &[Reach]) -> Reach {
    let mut result = Reach::default();
    for (kind, origins) in returned.kinds.iter().enumerate() {
        let Some(target) = result.kinds.get_mut(kind) else {
            continue;
        };
        target.source |= origins.source;
        for argument in origins.positions().filter_map(|position| arguments.get(position)) {
            if let Some(carried) = argument.kinds.get(kind) {
                target.absorb(*carried);
            }
        }
    }
    result
}
