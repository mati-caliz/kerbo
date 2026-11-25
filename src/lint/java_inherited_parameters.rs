use tree_sitter::Node;

use super::java::{METHOD, NAME_FIELD};
use super::java_calls::annotated_with;
use super::java_null_origin::NullOrigin;
use crate::syntax::{named_children, one_based};

const OVERRIDE: &str = "Override";
const PARAMETERS_FIELD: &str = "parameters";
const FORMAL_PARAMETER: &str = "formal_parameter";
const DIMENSIONS_FIELD: &str = "dimensions";

pub(super) struct InheritedParameter {
    pub(super) name: String,
    pub(super) origin: NullOrigin,
}

// Los parámetros de un método con `@Override`: si el método de arriba declara el parámetro
// `@Nullable`, también este puede llegar null. Se sabe en el índice, con la posición del método.
pub(super) fn inherited_parameters(method: Node<'_>, source: &[u8]) -> Vec<InheritedParameter> {
    if method.kind() != METHOD || !annotated_with(method, OVERRIDE, source) {
        return Vec::new();
    }
    let Some(name) = method.child_by_field_name(NAME_FIELD) else {
        return Vec::new();
    };
    let start = name.start_position();
    let (line, column) = (one_based(start.row), one_based(start.column));
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter(|(_, parameter)| {
            parameter.kind() == FORMAL_PARAMETER && parameter.child_by_field_name(DIMENSIONS_FIELD).is_none()
        })
        .filter_map(|(index, parameter)| {
            let parameter_name = parameter.child_by_field_name(NAME_FIELD)?;
            Some(InheritedParameter {
                name: source_text(parameter_name, source),
                origin: NullOrigin::InheritedParameter { line, column, index },
            })
        })
        .collect()
}

fn source_text(node: Node<'_>, source: &[u8]) -> String {
    super::java::text(node, source).to_owned()
}
