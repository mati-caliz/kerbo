use tree_sitter::Node;

use crate::lint::java::VARIABLE_DECLARATOR;
use crate::lint::java_null_inference::{is_overridable, returns_null};
use crate::lint::java_owner::annotation_names;
use crate::lint::java_type_variables::{has_nullable_qualified_type, has_nullable_type_variable};

const NULLABLE_ANNOTATIONS: &[&str] = &["Nullable", "CheckForNull"];
const NON_NULL_ANNOTATIONS: &[&str] = &["NonNull", "NotNull", "Nonnull"];
const PARAMETRIC_NULLNESS: &[&str] = &["ParametricNullness"];
const FORMAL_PARAMETER: &str = "formal_parameter";
const DIMENSIONS_FIELD: &str = "dimensions";
const TYPE_FIELD: &str = "type";
const ARRAY_TYPE: &str = "array_type";

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

// `E[] arreglo` (no un varargs): aunque `E` admita null, el arreglo no.
fn is_plain_array(name: Node<'_>) -> bool {
    name.parent().is_some_and(|parameter| {
        parameter.kind() == FORMAL_PARAMETER
            && (parameter.child_by_field_name(DIMENSIONS_FIELD).is_some()
                || parameter
                    .child_by_field_name(TYPE_FIELD)
                    .is_some_and(|written| written.kind() == ARRAY_TYPE))
    })
}

pub(super) fn parameter_nullness(name: Node<'_>, source: &[u8]) -> ParameterNullness {
    if declares_nullness(name, source, NULLABLE_ANNOTATIONS) || has_nullable_qualified_type(name, source) {
        ParameterNullness::Nullable
    } else if declares_nullness(name, source, NON_NULL_ANNOTATIONS) {
        ParameterNullness::NonNull
    } else if !is_plain_array(name)
        && (has_nullable_type_variable(name, source) || declares_nullness(name, source, PARAMETRIC_NULLNESS))
    {
        ParameterNullness::NullableVariable
    } else {
        ParameterNullness::Unannotated
    }
}

// Lo que IntelliJ infiere de nulos en un método: nada si se puede redefinir, y si no, si devuelve
// `null` sin un contrato que lo explique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullInference {
    Overridable,
    Closed,
    ClosedReturnsNull,
}

pub(super) fn inference_of(method: Option<Node<'_>>, source: &[u8]) -> NullInference {
    if method.is_none_or(is_overridable) {
        NullInference::Overridable
    } else if method.is_some_and(|declaration| returns_null(declaration, source)) {
        NullInference::ClosedReturnsNull
    } else {
        NullInference::Closed
    }
}
