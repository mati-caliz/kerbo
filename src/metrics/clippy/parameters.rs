use tree_sitter::Node;

use super::{children_of_kind, modifiers};

const PARAMETERS_FIELD: &str = "parameters";
const TRAIT_FIELD: &str = "trait";
const COUNTED: &[&str] = &["parameter", "self_parameter", "variadic_parameter"];
const IMPLEMENTATION: &str = "impl_item";
const FOREIGN_MODULE: &str = "foreign_mod_item";
const EXTERN: &str = "extern_modifier";
const ABI: &str = "string_literal";
const RUST_ABI: &str = "\"Rust\"";

// Calca too_many_arguments de clippy: el receptor cuenta, y no se miran las implementaciones de un
// trait (la firma la decide el trait), las funciones de un bloque extern ni las de otra ABI.
pub(super) fn too_many_arguments(function: Node<'_>, source: &[u8]) -> Option<usize> {
    let container = function.parent().and_then(|list| list.parent());
    let implements_trait = container.is_some_and(|container| {
        container.kind() == IMPLEMENTATION && container.child_by_field_name(TRAIT_FIELD).is_some()
    });
    let is_foreign = container.is_some_and(|container| container.kind() == FOREIGN_MODULE);
    if implements_trait || is_foreign || has_foreign_abi(function, source) {
        return None;
    }
    let list = function.child_by_field_name(PARAMETERS_FIELD)?;
    let mut cursor = list.walk();
    Some(
        list.named_children(&mut cursor)
            .filter(|parameter| COUNTED.contains(&parameter.kind()))
            .count(),
    )
}

fn has_foreign_abi(function: Node<'_>, source: &[u8]) -> bool {
    modifiers(function)
        .into_iter()
        .filter(|modifier| modifier.kind() == EXTERN)
        .any(|extern_modifier| {
            let abi = children_of_kind(extern_modifier, ABI)
                .first()
                .and_then(|abi| abi.utf8_text(source).ok());
            abi != Some(RUST_ABI)
        })
}
