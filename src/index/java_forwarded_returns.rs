use super::java::{Declaration, JavaSummary, NullInference};
use super::java_dereference::project_candidates;
use super::java_hardcoded_contracts::has_hardcoded_contract;
use super::java_null_origins::may_be_null;
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use super::java_variable_returns::enclosing_owner;
use crate::jvm::library::Library;
use crate::jvm::null_results::returns_null_for_null;
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
    let returned = returned_parameter(types, library, summary, chain);
    let library_class = called_library_class(types, library, summary, chain);
    if returned.is_none() && library_class.is_none() {
        return false;
    }
    chain.forwarded.iter().any(|(position, origin)| {
        may_be_null(types, library, summary, origin)
            && (returned == Some(*position)
                || library_class
                    .as_deref()
                    .is_some_and(|class| library_returns_null_for_null(library, class, chain, *position)))
    })
}

fn called_library_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Option<String> {
    let ([_], ChainRoot::Class(written)) = (chain.steps.as_slice(), &chain.root) else {
        return None;
    };
    match resolve(types, library, summary, written) {
        Resolved::Library(class) => Some(class),
        Resolved::Project(_) => None,
    }
}

// Un método estático de librería, llamado con su clase, al que IntelliJ le infiere del bytecode el
// contrato `null -> null` en ese lugar (`FileUtils.toFile(url)`): con un argumento que puede ser
// null, lo que devuelve también. Todas las sobrecargas de esa cantidad de parámetros lo cumplen.
fn library_returns_null_for_null(library: &Library, class: &str, chain: &Chain, position: usize) -> bool {
    let [step] = chain.steps.as_slice() else {
        return false;
    };
    library
        .declared_overloads(class, &step.name, step.arity)
        .is_some_and(|(declaring, overloads)| {
            !has_hardcoded_contract(&declaring.name, &step.name, step.arity)
                && overloads.iter().all(|method| {
                    method.is_static() && returns_null_for_null(library, &declaring.name, method, position)
                })
        })
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
