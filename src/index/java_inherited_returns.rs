use super::java::Declaration;
use super::java_library_arguments::NON_NULL_ANNOTATIONS;
use super::java_nullness::{ProjectTypes, library_ancestors, project_lineage};
use crate::jvm::library::Library;
use crate::jvm::method::Method;
use crate::lint::java_nullable_fields::ReturnNullness;

const QUALIFIER_SEPARATOR: char = '.';
const ANNOTATION_MARK: char = '@';
const NULLABLE_ANNOTATIONS: &[&str] = &["Nullable", "CheckForNull"];

// Como IntelliJ, un método sin anotación propia hereda la `@Nullable` o `@CheckForNull` del que
// redefine (`Jenkins.getItem(String)` redefine el `@CheckForNull` de `ItemGroup`), sea de una clase
// del proyecto o de un jar. Con varios tipos del mismo nombre simple no se sabe cuál es el de arriba.
pub(super) fn overrides_nullable(
    types: &ProjectTypes<'_>,
    library: &Library,
    declaration: &Declaration,
) -> bool {
    let declares_non_null = declaration
        .annotations
        .iter()
        .any(|annotation| NON_NULL_ANNOTATIONS.contains(&simple_name(annotation)));
    let owner = &declaration.owner.name;
    if declares_non_null || declaration.is_constructor || types.is_ambiguous(owner) {
        return false;
    }
    let in_project = project_lineage(types, library, owner)
        .iter()
        .skip(1)
        .any(|ancestor| {
            types.declarations_of(ancestor).any(|(_, inherited)| {
                inherited.return_nullness == ReturnNullness::Nullable
                    && has_same_signature(inherited, declaration)
            })
        });
    in_project
        || library_ancestors(types, library, owner).iter().any(|class| {
            let overloads = library.overloads(class, &declaration.name, declaration.parameters.len());
            !overloads.is_empty() && overloads.iter().all(is_annotated_nullable)
        })
}

// El mismo nombre y los mismos tipos escritos: un parámetro genérico que la clase de abajo concreta
// no coincide, y la anotación no se hereda.
fn has_same_signature(inherited: &Declaration, declaration: &Declaration) -> bool {
    !inherited.is_constructor
        && inherited.name == declaration.name
        && inherited.arity == declaration.arity
        && inherited.parameters.len() == declaration.parameters.len()
        && inherited
            .parameters
            .iter()
            .zip(&declaration.parameters)
            .all(|(above, below)| above.type_name == below.type_name)
}

// Si el jar anota el retorno `@Nullable` o `@CheckForNull`.
pub(super) fn is_annotated_nullable(method: &Method) -> bool {
    method
        .return_annotations
        .iter()
        .any(|annotation| NULLABLE_ANNOTATIONS.contains(&simple_name(annotation)))
}

fn simple_name(annotation: &str) -> &str {
    annotation
        .trim_start_matches(ANNOTATION_MARK)
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(annotation)
}
