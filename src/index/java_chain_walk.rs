use super::java::JavaSummary;
use super::java_dereference::{project_step, resolve_root};
use super::java_exact_classes::exact_class;
use super::java_inherited_returns::is_annotated_nullable;
use super::java_jdk::{is_always_defined, jdk_method};
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors};
use super::java_optional_returns::with_absent_alternatives;
use super::java_substituted_returns::returns_nullable_argument;
use super::java_variable_returns::enclosing_owner;
use crate::jvm::library::Library;
use crate::lint::java_chains::{Chain, ChainStep};

// Por cada paso de la cadena que se pudo resolver en la librería, si devuelve `@Nullable`. El tipo
// de cada paso sale de la firma del anterior; con sobrecargas del mismo nombre y aridad, sólo si
// todas dicen lo mismo. Lo del JDK que IntelliJ da por posiblemente null va primero: `ct.sym` no
// trae anotaciones.
pub(super) fn nullable_steps(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Vec<bool> {
    with_absent_alternatives(chain, walk(types, library, summary, chain).0)
}

// El tipo de lo que devuelve la cadena entera, si todos sus pasos se pudieron resolver.
pub(super) fn chain_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Option<Resolved> {
    walk(types, library, summary, chain).1
}

fn walk(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> (Vec<bool>, Option<Resolved>) {
    let mut current = enclosing_owner(types, library, chain)
        .map(Resolved::Project)
        .or_else(|| exact_class(types, library, summary, chain.exact_root.as_ref()?))
        .unwrap_or_else(|| resolve_root(types, library, summary, &chain.root));
    let mut nullable = Vec::new();
    for (index, step) in chain.steps.iter().enumerate() {
        let substituted: &[usize] = if index == 0 {
            &chain.nullable_root_arguments
        } else {
            &[]
        };
        let Some((is_nullable, returned)) = next_step(types, library, &current, (step, substituted)) else {
            return (nullable, None);
        };
        nullable.push(is_nullable);
        let Some(returned) = returned else {
            return (nullable, None);
        };
        current = returned;
    }
    (nullable, Some(current))
}

// Si el paso puede devolver null y el tipo de lo que devuelve (si se sabe). `None` si el paso no se
// pudo resolver.
fn next_step(
    types: &ProjectTypes<'_>,
    library: &Library,
    current: &Resolved,
    (step, substituted): (&ChainStep, &[usize]),
) -> Option<(bool, Option<Resolved>)> {
    let substitutes_null = || returns_nullable_argument(types, library, current, (step, substituted));
    if let Resolved::Project(owner) = current
        && let Some((is_nullable, returned)) = project_step(types, library, owner, step)
    {
        return Some((is_nullable || substitutes_null(), returned));
    }
    let classes = match current {
        Resolved::Library(class) => library.supertypes(class),
        Resolved::Project(name) => library_ancestors(types, library, name),
    };
    if let Some((returned, is_nullable)) = jdk_method(&classes, &step.name, step.arity) {
        let may_be_absent = is_nullable && !is_always_defined(&classes, &step.name, &step.text_arguments);
        return Some((may_be_absent, Some(Resolved::Library(returned.to_owned()))));
    }
    let overloads = classes
        .iter()
        .map(|class| library.overloads(class, &step.name, step.arity))
        .find(|found| !found.is_empty())
        .unwrap_or_default();
    let by_class_arguments: Vec<_> = overloads
        .iter()
        .filter(|method| {
            step.class_arguments
                .iter()
                .all(|position| method.takes_class_at(*position))
        })
        .cloned()
        .collect();
    let candidates = if by_class_arguments.is_empty() {
        overloads
    } else {
        by_class_arguments
    };
    let first = candidates.first()?;
    let is_nullable = candidates.iter().all(is_annotated_nullable) || substitutes_null();
    let returned = first
        .return_class()
        .filter(|returned| {
            candidates
                .iter()
                .all(|method| method.return_class().as_ref() == Some(returned))
        })
        .map(Resolved::Library);
    Some((is_nullable, returned))
}
