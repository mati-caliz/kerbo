use super::java::JavaSummary;
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::class_file::binary_to_source;
use crate::jvm::library::Library;

const UNRESOLVED: &str = "";
const OBJECT_START: char = 'L';
const OBJECT_END: char = ';';

// El tipo del campo de una variable (`j.jenkins` con `JenkinsRule j`): el que declara la clase del
// proyecto o una de arriba, o el del `.class` de la librería (o de sus padres). Si no se encuentra,
// la cadena no sigue.
pub(super) fn member_type(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (owner, field): (&str, &str),
) -> Resolved {
    let found = match resolve(types, library, summary, owner) {
        Resolved::Project(class) => project_lineage(types, library, &class)
            .iter()
            .find_map(|ancestor| types.field_of(ancestor, field))
            .map(|(declaring, declared)| resolve(types, library, declaring, &declared.type_name)),
        Resolved::Library(class) => library_field(types, library, &class, field),
    };
    found.unwrap_or_else(|| Resolved::Library(UNRESOLVED.to_owned()))
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
