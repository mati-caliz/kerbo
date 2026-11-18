use std::collections::BTreeSet;
use std::rc::Rc;

use super::instructions::CallTarget;
use super::null_partial::{Partial, PassedParameter, TooComplex, join, meet, passed};

fn parameter(name: &str, index: usize) -> PassedParameter {
    PassedParameter {
        target: Rc::new(CallTarget {
            owner: "sample/Checks".to_owned(),
            name: name.to_owned(),
            descriptor: "(Ljava/lang/String;)V".to_owned(),
        }),
        index,
        stable_call: true,
    }
}

fn call(name: &str) -> Partial {
    passed(parameter(name, 0))
}

fn sizes(partial: &Partial) -> Vec<usize> {
    match partial {
        Partial::Conditional(products) => products.iter().map(BTreeSet::len).collect(),
        Partial::Identity | Partial::Return | Partial::Dereference => Vec::new(),
    }
}

// Dos caminos: uno que no aporta no cambia nada, uno que termina bien gana siempre, y una
// desreferencia sólo queda si el otro también lo es. Dos llamadas quedan como dos alternativas.
#[test]
fn joining_paths_keeps_the_weakest_outcome() {
    assert_eq!(
        join(Partial::Identity, Partial::Dereference),
        Ok(Partial::Dereference)
    );
    assert_eq!(
        join(Partial::Dereference, Partial::Identity),
        Ok(Partial::Dereference)
    );
    assert_eq!(join(Partial::Return, Partial::Dereference), Ok(Partial::Return));
    assert_eq!(join(call("first"), Partial::Return), Ok(Partial::Return));
    assert_eq!(join(Partial::Dereference, call("first")), Ok(call("first")));
    assert_eq!(join(call("first"), Partial::Dereference), Ok(call("first")));
    let both = join(call("first"), call("second")).expect("entra en el límite");
    assert_eq!(sizes(&both), [1, 1]);
}

// Una instrucción y lo que sigue: una desreferencia en cualquiera de las dos alcanza, lo que sigue
// manda si la instrucción no hizo nada, y dos llamadas seguidas son una sola alternativa con las dos.
#[test]
fn meeting_an_instruction_with_what_follows_keeps_the_strongest_outcome() {
    assert_eq!(meet(Partial::Identity, Partial::Return), Ok(Partial::Return));
    assert_eq!(
        meet(Partial::Return, Partial::Dereference),
        Ok(Partial::Dereference)
    );
    assert_eq!(meet(call("first"), Partial::Return), Ok(call("first")));
    assert_eq!(
        meet(Partial::Dereference, Partial::Identity),
        Ok(Partial::Dereference)
    );
    assert_eq!(
        meet(call("first"), Partial::Dereference),
        Ok(Partial::Dereference)
    );
    assert_eq!(meet(call("first"), Partial::Identity), Ok(Partial::Identity));
    let together = meet(call("first"), call("second")).expect("entra en el límite");
    assert_eq!(sizes(&together), [2]);
}

// Como IntelliJ, con más de treinta llamadas pendientes se deja de inferir.
#[test]
fn too_many_pending_calls_are_too_complex() {
    let mut joined = Partial::Identity;
    let mut outcome = Ok(Partial::Identity);
    for index in 0..=30 {
        outcome = join(joined.clone(), passed(parameter("many", index)));
        if let Ok(partial) = &outcome {
            joined = partial.clone();
        }
    }
    assert_eq!(outcome, Err(TooComplex));
    assert_eq!(sizes(&joined).len(), 30);
}
