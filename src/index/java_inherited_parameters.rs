use super::java::{Declaration, JavaSummary};
use super::java_inherited_returns::has_same_signature;
use super::java_nullness::{ProjectTypes, project_lineage};
use super::java_parameter_nullness::ParameterNullness;
use crate::jvm::library::Library;

// Si el método que está en esa posición redefine a uno del proyecto que declara `@Nullable` el
// parámetro, y el de abajo no dice nada: IntelliJ le hereda la anotación. Con varios tipos del
// mismo nombre simple no se sabe cuál es el de arriba.
pub(super) fn overrides_nullable_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (line, column, index): (usize, usize, usize),
) -> bool {
    let Some(declaration) = summary
        .declarations
        .iter()
        .find(|declaration| declaration.position.line == line && declaration.position.column == column)
    else {
        return false;
    };
    let owner = &declaration.owner.name;
    let is_unannotated = declaration
        .parameters
        .get(index)
        .is_some_and(|parameter| parameter.nullness == ParameterNullness::Unannotated);
    if !is_unannotated || types.is_ambiguous(owner) {
        return false;
    }
    project_lineage(types, library, owner)
        .iter()
        .skip(1)
        .any(|ancestor| {
            types
                .declarations_of(ancestor)
                .any(|(_, inherited)| declares_nullable(inherited, declaration, index))
        })
}

fn declares_nullable(inherited: &Declaration, declaration: &Declaration, index: usize) -> bool {
    has_same_signature(inherited, declaration)
        && inherited
            .parameters
            .get(index)
            .is_some_and(|parameter| parameter.nullness == ParameterNullness::Nullable)
}
