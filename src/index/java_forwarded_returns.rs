use super::java::{Declaration, JavaSummary, NullInference};
use super::java_dereference::project_candidates;
use super::java_null_origins::may_be_null;
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use super::java_variable_returns::enclosing_owner;
use crate::jvm::library::Library;
use crate::lint::java_calls::Body;
use crate::lint::java_chains::{Chain, ChainRoot};

const QUALIFIER_SEPARATOR: char = '.';

// Un método del proyecto que nadie puede redefinir y cuyo cuerpo es sólo `return parametro;`:
// IntelliJ le infiere un contrato que devuelve ese argumento, así que una llamada sin receptor es
// tan nula como lo que recibe en ese lugar (`uncheckedCastNullableTToT(valor)`).
pub(super) fn forwards_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> bool {
    if chain.forwarded.is_empty() {
        return false;
    }
    let Some(returned) = returned_parameter(types, library, summary, chain) else {
        return false;
    };
    chain
        .forwarded
        .iter()
        .any(|(position, origin)| *position == returned && may_be_null(types, library, summary, origin))
}

// La posición del parámetro que devuelven todas las sobrecargas que la llamada puede elegir: en la
// clase que la encierra (o una de arriba) o, si no, en la de un `import static`.
fn returned_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Option<usize> {
    let ([step], ChainRoot::Class(innermost)) = (chain.steps.as_slice(), &chain.root) else {
        return None;
    };
    let enclosing =
        enclosing_owner(types, library, chain).or_else(|| project_class(types, library, summary, innermost));
    let candidates = enclosing
        .and_then(|owner| project_candidates(types, library, &owner, step))
        .or_else(|| {
            let imported = imported_owner(types, library, summary, &step.name)?;
            project_candidates(types, library, &imported, step)
        })?;
    let mut positions = candidates
        .iter()
        .map(|(_, declaration)| returned_position(declaration));
    let first = positions.next().flatten()?;
    positions.all(|other| other == Some(first)).then_some(first)
}

fn returned_position(declaration: &Declaration) -> Option<usize> {
    if declaration.inference == NullInference::Overridable || declaration.body != Body::Written {
        return None;
    }
    let returned = declaration.returned_field.as_deref()?;
    declaration
        .parameters
        .iter()
        .position(|parameter| parameter.name == returned)
}

fn project_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    written: &str,
) -> Option<String> {
    match resolve(types, library, summary, written) {
        Resolved::Project(owner) => Some(owner),
        Resolved::Library(_) => None,
    }
}

fn imported_owner(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    method: &str,
) -> Option<String> {
    let owner = summary.static_imports.iter().find_map(|path| {
        let (owner, member) = path.rsplit_once(QUALIFIER_SEPARATOR)?;
        (member == method).then_some(owner)
    })?;
    project_class(types, library, summary, owner)
}
