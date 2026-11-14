use tree_sitter::Node;

use crate::lint::java::VARIABLE_DECLARATOR;
use crate::lint::java_owner::annotation_names;
use crate::lint::java_type_variables::{has_nullable_qualified_type, has_nullable_type_variable};

const NULLABLE_ANNOTATIONS: &[&str] = &["Nullable", "CheckForNull"];
const NON_NULL_ANNOTATIONS: &[&str] = &["NonNull", "NotNull", "Nonnull"];
const PARAMETRIC_NULLNESS: &[&str] = &["ParametricNullness"];

// Lo que el parámetro dice de su nulabilidad: una anotación (`@Nullable`, `@NonNull`...), nada, o
// nada pero con un tipo que es una variable que admite null (`<T extends @Nullable Object>`, o una
// anotada `@ParametricNullness`, que admite null si su límite lo admite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterNullness {
    Unannotated,
    Nullable,
    NonNull,
    NullableVariable,
}

impl ParameterNullness {
    pub const fn is_annotated(self) -> bool {
        matches!(self, Self::Nullable | Self::NonNull)
    }
}

// Si el parámetro lleva alguna de estas anotaciones (`@Nullable`, `@NonNull`, `@NotNull`...).
fn declares_nullness(name: Node<'_>, source: &[u8], annotations: &[&str]) -> bool {
    name.parent()
        .and_then(|parent| {
            if parent.kind() == VARIABLE_DECLARATOR {
                parent.parent()
            } else {
                Some(parent)
            }
        })
        .map(|parameter| annotation_names(parameter, source))
        .unwrap_or_default()
        .iter()
        .any(|annotation| annotations.contains(&annotation.as_str()))
}

pub(super) fn parameter_nullness(name: Node<'_>, source: &[u8]) -> ParameterNullness {
    if declares_nullness(name, source, NULLABLE_ANNOTATIONS) || has_nullable_qualified_type(name, source) {
        ParameterNullness::Nullable
    } else if declares_nullness(name, source, NON_NULL_ANNOTATIONS) {
        ParameterNullness::NonNull
    } else if has_nullable_type_variable(name, source) || declares_nullness(name, source, PARAMETRIC_NULLNESS)
    {
        ParameterNullness::NullableVariable
    } else {
        ParameterNullness::Unannotated
    }
}
