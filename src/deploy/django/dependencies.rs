use tree_sitter::Node;

use super::model::ModelKey;
use super::operations::MigrationKey;
use super::settings::Settings;
use super::values::{elements, text};
use crate::deploy::python_calls::Call;

pub const FIRST: &str = "__first__";
pub const LATEST: &str = "__latest__";
const SWAPPABLE: &str = "swappable_dependency";
const SWAPPER_MODULE: &str = "swapper";
const SWAPPER_MODEL_NAME: &str = "get_model_name";

pub fn keys(node: Node<'_>, source: &[u8], settings: &Settings) -> Vec<MigrationKey> {
    elements(node)
        .into_iter()
        .filter_map(|element| key(element, source, settings))
        .collect()
}

fn key(node: Node<'_>, source: &[u8], settings: &Settings) -> Option<MigrationKey> {
    if Call::parse(node, source).is_some_and(|call| call.method == SWAPPABLE) {
        return Some(MigrationKey::new(&settings.user_model.app, FIRST));
    }
    let parts = elements(node);
    let [app, name] = parts.as_slice() else {
        return None;
    };
    Some(MigrationKey::new(&text(*app, source)?, &text(*name, source)?))
}

pub fn swappable_default(node: Node<'_>, source: &[u8]) -> Option<ModelKey> {
    let call = Call::parse(node, source)?;
    if call.receiver != SWAPPER_MODULE || call.method != SWAPPER_MODEL_NAME {
        return None;
    }
    Some(ModelKey::new(
        &text(call.positional(0)?, source)?,
        &text(call.positional(1)?, source)?,
    ))
}
