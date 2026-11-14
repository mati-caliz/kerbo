use super::java::{Declaration, JavaSummary, Parameter};
use super::java_null_arguments::{Slot, accepts, declares_method_variable, parameter_at, project_owners};
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use crate::jvm::generics::{Generic, method_signature};
use crate::jvm::library::Library;
use crate::lint::java_nested_arguments::NestedNull;
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_receiver_arguments::ArgumentNullness;
use crate::lint::java_receivers::{Receiver, simple_type_name};
use crate::lint::java_type_arguments::TypeArgument;

// La variable de tipo del método de adentro que recibe el `null`: el tipo que devuelve el método,
// en qué lugar de sus argumentos de tipo aparece la variable, y su nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InferredVariable {
    returned: String,
    position: usize,
    name: String,
}

// Calca el "Passing 'null' argument to parameter annotated as non-null" de un `null` pasado a una
// variable de tipo del método (`immutableEntry(null, 1)`, `Collections.singletonMap(null, 1)`)
// cuando la llamada es argumento de otra cuyo parámetro pone en ese lugar algo no nulo:
// `builder.put(Entry<? extends K, ? extends V>)` con `Builder<String, Integer> builder`, o una
// variable del método sin límite `@Nullable` (`toMap(keys, Functions.constant(null))`). IntelliJ
// infiere la variable del de adentro desde el parámetro de afuera. Como los tipos del proyecto se
// indexan por nombre simple (varios `Builder`), de afuera cuentan los que reciben el tipo que devuelve
// el de adentro.
pub(super) fn context_inferred_non_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    nested: &NestedNull,
) -> Option<String> {
    let variable = inner_variable(types, library, summary, &nested.inner)?;
    outer_requires_non_null(types, library, summary, (&nested.outer, &variable)).then_some(variable.name)
}

fn inner_variable(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    inner: &NullArgument,
) -> Option<InferredVariable> {
    let library_class = match &inner.receiver {
        Receiver::Type(written) => match resolve(types, library, summary, written) {
            Resolved::Library(class) => Some(class),
            Resolved::Project(_) => None,
        },
        Receiver::Scope(_) | Receiver::Unknown => None,
    };
    let found: Vec<InferredVariable> = match library_class {
        Some(class) => library
            .overloads(&class, &inner.name, inner.arity)
            .iter()
            .map(|method| {
                let signature = method.signature.as_deref().and_then(method_signature)?;
                let Generic::Variable(name) = signature.arguments.get(inner.index)? else {
                    return None;
                };
                let Generic::Class(returned, arguments) = &signature.returned else {
                    return None;
                };
                let position = arguments
                    .iter()
                    .position(|argument| *argument == Generic::Variable(name.clone()))?;
                signature.parameters.contains(name).then(|| InferredVariable {
                    returned: simple_type_name(returned),
                    position,
                    name: name.clone(),
                })
            })
            .collect::<Option<_>>()?,
        None => project_candidates(types, library, summary, inner)
            .into_iter()
            .map(|(declaring, declaration, parameter)| {
                if !declares_method_variable(declaring, declaration, &parameter.type_name) {
                    return None;
                }
                let position = declaration
                    .returned_arguments
                    .iter()
                    .position(|argument| *argument == parameter.type_name)?;
                Some(InferredVariable {
                    returned: declaration.returned.clone(),
                    position,
                    name: parameter.type_name.clone(),
                })
            })
            .collect::<Option<_>>()?,
    };
    let first = found.first()?;
    found.iter().all(|other| other == first).then(|| first.clone())
}

fn outer_requires_non_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (outer, variable): (&NullArgument, &InferredVariable),
) -> bool {
    let receiver_marked = types.is_marked(summary, outer.receiver_arguments.marking);
    let candidates: Vec<_> = project_candidates(types, library, summary, outer)
        .into_iter()
        .filter(|(_, _, parameter)| parameter.type_name == variable.returned)
        .collect();
    !candidates.is_empty()
        && candidates.iter().all(|(declaring, declaration, parameter)| {
            types.is_null_marked(declaring, &declaration.owner)
                && match parameter.type_arguments.get(variable.position) {
                    Some(TypeArgument::Concrete | TypeArgument::MethodVariable) => true,
                    Some(TypeArgument::OtherVariable(name)) => {
                        receiver_marked
                            && receiver_substitutes_non_null((declaring, declaration), outer, name)
                    }
                    _ => false,
                }
        })
}

// Si el receptor de afuera se declara con un tipo no nulo en el lugar de esa variable de la clase.
fn receiver_substitutes_non_null(
    (declaring, declaration): (&JavaSummary, &Declaration),
    outer: &NullArgument,
    variable: &str,
) -> bool {
    let receiver = &outer.receiver_arguments;
    declaring
        .generics
        .types
        .iter()
        .find(|generics| generics.name == declaration.owner.name)
        .and_then(|generics| generics.parameters.iter().position(|name| name == variable))
        .and_then(|position| receiver.arguments.get(position))
        .is_some_and(|nullness| {
            matches!(
                nullness,
                ArgumentNullness::Concrete | ArgumentNullness::NonNullVariable
            )
        })
}

// Los métodos del proyecto que puede llamar la llamada, con el parámetro que recibe el argumento
// entero, de la clase más cercana que los declara.
fn project_candidates<'summary>(
    types: &ProjectTypes<'summary>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Vec<(&'summary JavaSummary, &'summary Declaration, &'summary Parameter)> {
    project_owners(types, library, summary, argument)
        .iter()
        .find_map(|owner| {
            let found: Vec<_> = types
                .declarations_of(owner)
                .filter(|(_, declaration)| {
                    declaration.is_constructor == argument.is_constructor
                        && declaration.name == argument.name
                        && accepts(declaration, argument.arity)
                })
                .filter_map(|(declaring, declaration)| {
                    let (parameter, slot) = parameter_at(declaration, argument)?;
                    (slot == Slot::Whole).then_some((declaring, declaration, parameter))
                })
                .collect();
            (!found.is_empty()).then_some(found)
        })
        .unwrap_or_default()
}
