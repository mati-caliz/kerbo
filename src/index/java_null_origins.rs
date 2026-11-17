use super::java::{Declaration, JavaSummary};
use super::java_dereference::may_return_null;
use super::java_null_arguments::{accepts, project_owners};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::java_contracts::Constraint;
use crate::lint::java_fields::Volatility;
use crate::lint::java_null_flow::NullOrigin;
use crate::lint::java_null_inference::NullArgument;

// Si lo que dio valor a la variable puede ser null: un `null` escrito, un campo o parámetro
// `@Nullable`, una llamada que puede devolverlo o el campo `@Nullable` de otra variable.
pub(super) fn may_be_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    origin: &NullOrigin,
) -> bool {
    match origin {
        NullOrigin::Call(chain) => {
            may_return_null(types, library, summary, chain)
                && !chain.steps.last().is_some_and(|last| last.checked)
        }
        NullOrigin::Annotated | NullOrigin::Literal => true,
        NullOrigin::QualifiedField {
            owner,
            via,
            field,
            after_call,
        } => is_nullable_field(
            types,
            library,
            summary,
            (owner, via.as_deref(), field, *after_call),
        ),
    }
}

// Si el campo que declara la clase del proyecto (o una de arriba en el proyecto) es `@Nullable` y,
// leído de nuevo después de una llamada, si es `volatile`. Con un campo arreglo en el medio, el
// campo es del tipo de sus elementos.
fn is_nullable_field(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (owner, via, field, after_call): (&str, Option<&str>, &str, bool),
) -> bool {
    let Resolved::Project(mut owner) = resolve(types, library, summary, owner) else {
        return false;
    };
    if let Some(via) = via {
        let Some((declaring, array)) = project_lineage(types, library, &owner)
            .iter()
            .find_map(|class| types.field_of(class, via))
        else {
            return false;
        };
        let Resolved::Project(element) = resolve(types, library, declaring, &array.type_name) else {
            return false;
        };
        owner = element;
    }
    project_lineage(types, library, &owner)
        .iter()
        .find_map(|class| types.field_of(class, field))
        .is_some_and(|(_, declared)| {
            declared.is_nullable && (!after_call || declared.volatility == Volatility::Volatile)
        })
}

// Si el método del proyecto que recibe el argumento siempre falla con null en ese lugar (una cláusula
// `fail` inferida de su cuerpo, como `checkGoodName(name)` con `if (name == null) throw ...`): después
// de la llamada el valor ya no es null, aunque el parámetro no sea no nulo.
pub(super) fn fails_on_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> bool {
    project_owners(types, library, summary, argument)
        .iter()
        .find_map(|owner| {
            let found: Vec<&Declaration> = types
                .declarations_of(owner)
                .map(|(_, declaration)| declaration)
                .filter(|declaration| {
                    declaration.is_constructor == argument.is_constructor
                        && declaration.name == argument.name
                        && accepts(declaration, argument.arity)
                })
                .collect();
            (!found.is_empty()).then_some(found)
        })
        .is_some_and(|candidates| {
            candidates.iter().all(|declaration| {
                declaration
                    .fail_clauses
                    .iter()
                    .any(|clause| fails_only_on_null(clause, argument.index))
            })
        })
}

// Una cláusula que falla con null en esa posición, sea lo que sea el resto.
fn fails_only_on_null(clause: &[Option<Constraint>], index: usize) -> bool {
    clause.iter().enumerate().all(|(position, constraint)| {
        if position == index {
            *constraint == Some(Constraint::Null)
        } else {
            constraint.is_none()
        }
    })
}
