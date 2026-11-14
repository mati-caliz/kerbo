use super::java::JavaSummary;
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::generics::{Generic, class_signature, method_signature};
use crate::jvm::library::Library;
use crate::jvm::method::Method;
use crate::lint::java_called_receivers::CalledReceiver;
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_receiver_arguments::{ArgumentNullness, InheritedMember};
use crate::lint::java_receivers::Receiver;

// Bajo `@NullMarked` del que llama, el parámetro de un método de librería cuyo tipo es una variable
// de la clase no admite null si el receptor se declara con un tipo no nulo en su lugar
// (`HashMap<Object, Object> map` en `map.put(i, null)`, `ConcurrentMap<K, V>` con `K` y `V` sin
// límite `@Nullable`). Con sobrecargas, todas tienen que coincidir.
// Los parámetros del JDK que las anotaciones externas de IntelliJ marcan `@Nullable`: clase,
// método, cantidad de argumentos y posición.
const LIBRARY_NULLABLE: &[(&str, &str, usize, usize)] = &[("java.util.Optional", "orElse", 1, 0)];

pub(super) fn library_substituted_non_null(
    library: &Library,
    class: &str,
    argument: &NullArgument,
    caller_marked: bool,
) -> Option<String> {
    let receiver = &argument.receiver_arguments;
    let annotated_nullable = LIBRARY_NULLABLE.iter().any(|(owner, name, arity, index)| {
        *owner == class && *name == argument.name && *arity == argument.arity && *index == argument.index
    });
    if receiver.arguments.is_empty() || !caller_marked || annotated_nullable {
        return None;
    }
    let variables = class_variables(library, class)?;
    let (declaring, methods) = library.declared_overloads(class, &argument.name, argument.arity)?;
    let declaring_variables = declaring
        .signature
        .as_deref()
        .and_then(class_signature)
        .map(|signature| signature.parameters)?;
    if declaring_variables != variables {
        return None;
    }
    let mut names = methods.iter().map(|method| {
        let signature = method.signature.as_deref().and_then(method_signature)?;
        let Generic::Variable(variable) = signature.arguments.get(argument.index)? else {
            return None;
        };
        if signature.parameters.contains(variable) {
            return None;
        }
        let position = variables.iter().position(|name| name == variable)?;
        matches!(
            receiver.arguments.get(position),
            Some(ArgumentNullness::Concrete | ArgumentNullness::NonNullVariable)
        )
        .then(|| variable.clone())
    });
    let first = names.next()??;
    names.all(|other| other.is_some()).then_some(first)
}

fn class_variables(library: &Library, class: &str) -> Option<Vec<String>> {
    library
        .class_or_jdk(class)?
        .signature
        .as_deref()
        .and_then(class_signature)
        .map(|signature| signature.parameters)
}

// La clase de librería de un receptor heredado: el tipo del campo o lo que devuelve el getter que
// declara la clase más cercana de las de arriba en el proyecto.
pub(super) fn inherited_library_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    member: &InheritedMember,
) -> Option<String> {
    let (declaring, written) = member
        .scope
        .iter()
        .flat_map(|owner| project_lineage(types, library, owner))
        .find_map(|class| declared_member_type(types, &class, member))?;
    match resolve(types, library, declaring, &written) {
        Resolved::Library(class) => Some(class),
        Resolved::Project(_) => None,
    }
}

fn declared_member_type<'summary>(
    types: &ProjectTypes<'summary>,
    class: &str,
    member: &InheritedMember,
) -> Option<(&'summary JavaSummary, String)> {
    if member.getter {
        return types
            .declarations_of(class)
            .find(|(_, declaration)| {
                !declaration.is_constructor
                    && declaration.name == member.name
                    && declaration.parameters.is_empty()
            })
            .map(|(declaring, declaration)| (declaring, declaration.returned.clone()));
    }
    types
        .field_of(class, &member.name)
        .map(|(declaring, field)| (declaring, field.type_name.clone()))
}

// El tipo que devuelve la llamada que hace de receptor (`Splitter.on(x)` devuelve `Splitter`), si
// todas las sobrecargas que puede llamar devuelven el mismo: del proyecto, por su nombre; de
// librería, con su paquete.
pub(super) fn called_type(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    called: &CalledReceiver,
) -> Option<String> {
    let Receiver::Type(written) = &called.receiver else {
        return None;
    };
    let returned: Vec<String> = match resolve(types, library, summary, written) {
        Resolved::Project(owner) => project_lineage(types, library, &owner)
            .iter()
            .map(|class| {
                types
                    .declarations_of(class)
                    .filter(|(_, declaration)| {
                        !declaration.is_constructor
                            && declaration.name == called.name
                            && declaration.parameters.len() == called.arity
                    })
                    .map(|(_, declaration)| declaration.returned.clone())
                    .collect::<Vec<_>>()
            })
            .find(|found| !found.is_empty())?,
        Resolved::Library(class) => library
            .overloads(&class, &called.name, called.arity)
            .iter()
            .map(Method::return_class)
            .collect::<Option<_>>()?,
    };
    let first = returned.first()?;
    returned.iter().all(|other| other == first).then(|| first.clone())
}
