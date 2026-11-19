use super::java::JavaSummary;
use super::java_argument_types::receiver_library_class;
use super::java_null_arguments::{accepts, project_owners};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::jvm::method::Method;
use crate::lint::java_calls::Body;
use crate::lint::java_null_inference::NullArgument;

const OBJECT: &str = "java.lang.Object";

// El tipo del parámetro sin tipo de una lambda: el del parámetro, en esa posición, del único método
// abstracto de la interfaz funcional que recibe la llamada (`sesion.then(r -> ...)` con
// `then(Step)` y `Step.run(JenkinsRule)`). Si la llamada tiene sobrecargas que no coinciden en esa
// interfaz, o la interfaz usa ahí una variable de tipo, no se sabe.
pub(super) fn lambda_parameter_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (slot, position): (&NullArgument, usize),
) -> Option<Resolved> {
    let interface = project_interface(types, library, summary, slot)
        .or_else(|| library_interface(types, library, summary, slot))?;
    match interface {
        Resolved::Project(name) => {
            let lineage = project_lineage(types, library, &name);
            let mut methods = lineage.iter().flat_map(|ancestor| {
                types
                    .declarations_of(ancestor)
                    .filter(|(_, declaration)| declaration.body == Body::Abstract)
            });
            let (declaring, method) = methods.next()?;
            if methods.next().is_some() {
                return None;
            }
            let parameter = method.parameters.get(position)?;
            Some(resolve(types, library, declaring, &parameter.type_name))
        }
        Resolved::Library(name) => {
            let parameter = single_abstract_method(library, &name)?
                .parameter_type(position)
                .filter(|parameter| parameter != OBJECT)?;
            Some(
                types
                    .qualified(&parameter)
                    .map_or(Resolved::Library(parameter), Resolved::Project),
            )
        }
    }
}

// El único método abstracto de la interfaz de librería, propio o heredado de otra
// (`JenkinsSessionExtension.Step` no declara nada: su `run` viene de `JenkinsSessionFixture.Step`).
fn single_abstract_method(library: &Library, interface: &str) -> Option<Method> {
    let mut methods: Vec<Method> = library
        .supertypes(interface)
        .iter()
        .filter_map(|ancestor| library.class_or_jdk(ancestor))
        .flat_map(|class| {
            class
                .methods
                .iter()
                .filter(|method| method.is_abstract())
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect();
    methods.dedup_by(|first, second| first.name == second.name && first.descriptor == second.descriptor);
    let only = methods.pop()?;
    methods.is_empty().then_some(only)
}

// La interfaz que declara el parámetro del método del proyecto al que se le pasa la lambda.
fn project_interface(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    slot: &NullArgument,
) -> Option<Resolved> {
    let mut interfaces = project_owners(types, library, summary, slot)
        .iter()
        .find_map(|owner| {
            let found: Vec<Resolved> = types
                .declarations_of(owner)
                .filter(|(_, declaration)| {
                    declaration.is_constructor == slot.is_constructor
                        && declaration.name == slot.name
                        && accepts(declaration, slot.arity)
                })
                .filter_map(|(declaring, declaration)| {
                    let parameter = declaration.parameters.get(slot.index)?;
                    Some(resolve(types, library, declaring, &parameter.type_name))
                })
                .collect();
            (!found.is_empty()).then_some(found)
        })?
        .into_iter();
    let first = interfaces.next()?;
    interfaces.all(|other| other == first).then_some(first)
}

// Lo mismo cuando el método es de una librería: todas las sobrecargas de esa cantidad de
// parámetros reciben ahí la misma clase.
fn library_interface(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    slot: &NullArgument,
) -> Option<Resolved> {
    let class = receiver_library_class(types, library, summary, slot)?;
    let overloads = library.overloads(&class, &slot.name, slot.arity);
    let first = overloads.first()?.parameter_type(slot.index)?;
    overloads
        .iter()
        .all(|method| method.parameter_type(slot.index).as_ref() == Some(&first))
        .then_some(Resolved::Library(first))
}
