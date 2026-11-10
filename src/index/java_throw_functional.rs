use super::java::JavaSummary;
use super::java_nullness::Resolved;
use super::java_throw_lookup::{Signature, ThrowLookup, all_thrown};
use super::java_throw_wanted::Wanted;
use crate::lint::java_throws::{
    ArgumentKind, CONSTRUCTOR_REFERENCE, CallRoot, CallStep, ThrowOrigin, ThrowPoint,
};

// Lo que tiran las firmas candidatas de un paso. Un `throws E` que lleva una lambda o una
// referencia a método es lo que esa lambda o ese método tiran: si no tiran nada chequeado, el
// compilador infiere `RuntimeException` y la llamada no tira nada.
pub(super) fn step_thrown(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    signatures: &[Signature],
    step: &CallStep,
) -> Option<Vec<Resolved>> {
    let mut thrown = Vec::new();
    for signature in signatures {
        if signature.unresolved {
            return None;
        }
        thrown.extend(signature.thrown.iter().cloned());
        for position in signature.carried.iter().flatten() {
            thrown.extend(functional_thrown(
                lookup,
                summary,
                step.arguments.get(*position)?,
            )?);
        }
    }
    Some(thrown)
}

fn functional_thrown(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    argument: &ArgumentKind,
) -> Option<Vec<Resolved>> {
    match argument {
        ArgumentKind::Lambda(points) => lambda_thrown(lookup, summary, points),
        ArgumentKind::Reference(root, name) => referenced_thrown(lookup, summary, root, name),
        ArgumentKind::Unknown | ArgumentKind::Text | ArgumentKind::Charset => None,
    }
}

// Lo que sale del cuerpo de la lambda, sin lo que atrapa ella misma. Un relanzamiento puede llevar
// cualquier subtipo y el `E` inferido no se sabe.
fn lambda_thrown(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    points: &[ThrowPoint],
) -> Option<Vec<Resolved>> {
    let mut thrown = Vec::new();
    for point in points {
        if matches!(point.origin, ThrowOrigin::Rethrown(_)) {
            return None;
        }
        thrown.extend(
            lookup
                .thrown_by(summary, &point.origin)?
                .into_iter()
                .filter(|exception| !is_caught(lookup, summary, exception, &point.caught)),
        );
    }
    Some(thrown)
}

fn is_caught(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    exception: &Resolved,
    caught: &[String],
) -> bool {
    caught
        .iter()
        .any(|written| lookup.is_subclass(exception, &lookup.named(summary, written)) == Some(true))
}

// Sin saber qué sobrecarga elige la interfaz funcional, cuentan todas las que tienen ese nombre.
fn referenced_thrown(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    root: &CallRoot,
    name: &str,
) -> Option<Vec<Resolved>> {
    let signatures = if name == CONSTRUCTOR_REFERENCE {
        let CallRoot::Type(written) = root else {
            return None;
        };
        lookup.constructors(&lookup.named(summary, written), None)?
    } else {
        lookup.on_root(summary, root, Wanted::any_arity(name))?
    };
    all_thrown(&signatures)
}
