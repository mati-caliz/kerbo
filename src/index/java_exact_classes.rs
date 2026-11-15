use super::java::JavaSummary;
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::java_calls::{Arity, ReturnType};
use crate::lint::java_exact_roots::ExactRoot;

// La clase del proyecto con que se creó la raíz de la cadena: la del `new`, o la que devuelven todas
// las sobrecargas estáticas de la fábrica.
pub(super) fn exact_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    exact: &ExactRoot,
) -> Option<Resolved> {
    match exact {
        ExactRoot::Construction(class) => project_only(resolve(types, library, summary, class)),
        ExactRoot::Factory { class, method, arity } => {
            let Resolved::Project(owner) = resolve(types, library, summary, class) else {
                return None;
            };
            let mut returned = project_lineage(types, library, &owner)
                .iter()
                .flat_map(|ancestor| types.declarations_of(ancestor))
                .filter(|(_, declaration)| {
                    !declaration.is_constructor
                        && declaration.name == *method
                        && declaration.arity == Arity::Fixed
                        && declaration.parameters.len() == *arity
                })
                .map(|(declaring, declaration)| {
                    (declaration.return_type == ReturnType::Reference)
                        .then(|| resolve(types, library, declaring, &declaration.returned))
                })
                .collect::<Vec<_>>()
                .into_iter();
            let first = returned.next().flatten()?;
            returned
                .all(|other| other.as_ref() == Some(&first))
                .then_some(first)
                .and_then(project_only)
        }
    }
}

fn project_only(resolved: Resolved) -> Option<Resolved> {
    matches!(resolved, Resolved::Project(_)).then_some(resolved)
}
