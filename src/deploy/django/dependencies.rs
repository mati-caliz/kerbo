use tree_sitter::Node;

use super::model::ModelKey;
use super::operations::MigrationKey;
use super::settings::Settings;
use super::values::{elements, raw, text};
use crate::deploy::python_calls::Call;

pub const FIRST: &str = "__first__";
pub const LATEST: &str = "__latest__";
const SWAPPABLE: &str = "swappable_dependency";
const SWAPPER_MODULE: &str = "swapper";
const SWAPPER_MODEL_NAME: &str = "get_model_name";
const SWAPPER_SETTING: &str = "swappable_setting";
const SETTINGS_PREFIX: &str = "settings.";
const USER_MODEL_SETTING: &str = "AUTH_USER_MODEL";
const MODEL_SETTING_SUFFIX: &str = "_MODEL";
pub const SWAPPABLE_SETTING_APP: &str = "<settings>";

pub fn keys(node: Node<'_>, source: &[u8], settings: &Settings) -> Vec<MigrationKey> {
    elements(node)
        .into_iter()
        .filter_map(|element| key(element, source, settings))
        .collect()
}

fn key(node: Node<'_>, source: &[u8], settings: &Settings) -> Option<MigrationKey> {
    if let Some(call) = Call::parse(node, source).filter(|call| call.method == SWAPPABLE) {
        let setting = raw(call.positional(0)?, source);
        return setting
            .ends_with(USER_MODEL_SETTING)
            .then(|| MigrationKey::new(&settings.user_model.app, FIRST));
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

pub fn swappable_reference(node: Node<'_>, source: &[u8], settings: &Settings) -> Option<ModelKey> {
    let reference = raw(node, source);
    if reference.ends_with(USER_MODEL_SETTING) {
        return Some(settings.user_model.clone());
    }
    if let Some(setting) = reference
        .strip_prefix(SETTINGS_PREFIX)
        .filter(|setting| setting.ends_with(MODEL_SETTING_SUFFIX))
    {
        return Some(ModelKey::new(SWAPPABLE_SETTING_APP, setting));
    }
    swappable_default(node, source)
}

pub fn swappable_setting(node: Node<'_>, source: &[u8]) -> Option<String> {
    let Some(call) = Call::parse(node, source).filter(|call| call.method == SWAPPER_SETTING) else {
        return text(node, source).map(|setting| setting.to_lowercase());
    };
    let model = text(call.positional(1)?, source)?;
    Some(format!("_{model}{MODEL_SETTING_SUFFIX}").to_lowercase())
}
