use super::java::JavaSummary;
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, project_lineage, resolve};
use crate::jvm::class_file::binary_to_source;
use crate::jvm::library::Library;

const UNRESOLVED: &str = "";
const OBJECT_START: char = 'L';
const OBJECT_END: char = ';';

// El tipo al que se llega leyendo campos desde una variable (`j.jenkins` con `JenkinsRule j`,
// `r.jenkins.pluginManager`): el de cada campo es el que declara la clase del proyecto o una de
// arriba, o el del `.class` de la librería (o de sus padres). El primer campo se busca en cada
// dueño posible, en orden: un campo heredado llega con las clases que encierran al uso. Si no se
// encuentra, la cadena no sigue.
pub(super) fn member_type(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (owners, fields): (&[String], &[String]),
) -> Resolved {
    owners
        .iter()
        .find_map(|owner| {
            fields
                .iter()
                .try_fold(resolve(types, library, summary, owner), |current, field| {
                    field_type(types, library, &current, field)
                })
        })
        .unwrap_or_else(|| Resolved::Library(UNRESOLVED.to_owned()))
}

fn field_type(
    types: &ProjectTypes<'_>,
    library: &Library,
    owner: &Resolved,
    field: &str,
) -> Option<Resolved> {
    match owner {
        Resolved::Project(class) => project_lineage(types, library, class)
            .iter()
            .find_map(|ancestor| types.field_of(ancestor, field))
            .map(|(declaring, declared)| resolve(types, library, declaring, &declared.type_name))
            .or_else(|| {
                library_ancestors(types, library, class)
                    .iter()
                    .find_map(|ancestor| library_field(types, library, ancestor, field))
            }),
        Resolved::Library(class) => library_field(types, library, class, field),
    }
}

fn library_field(types: &ProjectTypes<'_>, library: &Library, class: &str, field: &str) -> Option<Resolved> {
    let descriptor = library.supertypes(class).iter().find_map(|ancestor| {
        library
            .class_or_jdk(ancestor)?
            .fields
            .iter()
            .find(|declared| declared.name == field)
            .map(|declared| declared.descriptor.clone())
    })?;
    let binary = descriptor.strip_prefix(OBJECT_START)?.strip_suffix(OBJECT_END)?;
    let qualified = binary_to_source(binary);
    Some(
        types
            .qualified(&qualified)
            .map_or(Resolved::Library(qualified), Resolved::Project),
    )
}
