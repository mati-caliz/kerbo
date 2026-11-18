use std::collections::BTreeSet;
use std::rc::Rc;

use super::instructions::CallTarget;

// El `EQUATION_SIZE_LIMIT` de IntelliJ: con más llamadas pendientes que esto, deja de inferir.
const EQUATION_SIZE_LIMIT: usize = 30;

// El parámetro pasado tal cual a otro método: es no nulo si el de allá lo es.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct PassedParameter {
    pub(super) target: Rc<CallTarget>,
    pub(super) index: usize,
    pub(super) stable_call: bool,
}

pub(super) type Products = BTreeSet<BTreeSet<PassedParameter>>;

// El `PResult` de IntelliJ (`Parameters.java`): qué pasa con un parámetro null de acá en adelante.
// `Conditional` es una suma de productos: todos los caminos (la suma) necesitan que alguna de sus
// llamadas (el producto) exija el parámetro no nulo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Partial {
    Identity,
    Return,
    Dereference,
    Conditional(Products),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TooComplex;

pub(super) fn passed(parameter: PassedParameter) -> Partial {
    Partial::Conditional(BTreeSet::from([BTreeSet::from([parameter])]))
}

fn limited(products: Products) -> Result<Partial, TooComplex> {
    let size: usize = products.iter().map(BTreeSet::len).sum();
    if size > EQUATION_SIZE_LIMIT {
        return Err(TooComplex);
    }
    Ok(Partial::Conditional(products))
}

// Dos caminos que salen del mismo lugar.
pub(super) fn join(first: Partial, second: Partial) -> Result<Partial, TooComplex> {
    match (first, second) {
        (Partial::Identity, other) | (other, Partial::Identity) => Ok(other),
        (Partial::Return, _) | (_, Partial::Return) => Ok(Partial::Return),
        (Partial::Dereference, other) | (other, Partial::Dereference) => Ok(other),
        (Partial::Conditional(mut products), Partial::Conditional(others)) => {
            products.extend(others);
            limited(products)
        }
    }
}

// Lo que hace una instrucción y lo que viene después de ella.
pub(super) fn meet(first: Partial, second: Partial) -> Result<Partial, TooComplex> {
    match (first, second) {
        (Partial::Identity | Partial::Return, other) | (other, Partial::Return) => Ok(other),
        (Partial::Dereference, _) | (_, Partial::Dereference) => Ok(Partial::Dereference),
        (Partial::Conditional(_), Partial::Identity) => Ok(Partial::Identity),
        (Partial::Conditional(products), Partial::Conditional(others)) => limited(
            products
                .iter()
                .flat_map(|product| others.iter().map(move |other| product | other))
                .collect(),
        ),
    }
}
