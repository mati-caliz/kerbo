use tree_sitter::Node;

use super::settings::Settings;
use super::values::{boolean, parenthesized_content};

const SETTINGS_OBJECT: &str = "settings";
const NEGATION: &str = "not_operator";
const ATTRIBUTE: &str = "attribute";

pub fn truth(node: Node<'_>, source: &[u8], settings: &Settings) -> Option<bool> {
    if let Some(content) = parenthesized_content(node) {
        return truth(content, source, settings);
    }
    match node.kind() {
        NEGATION => truth(node.child_by_field_name("argument")?, source, settings).map(|value| !value),
        ATTRIBUTE => {
            let object = node.child_by_field_name("object")?.utf8_text(source).ok()?;
            let name = node.child_by_field_name("attribute")?.utf8_text(source).ok()?;
            (object == SETTINGS_OBJECT).then(|| settings.flag(name)).flatten()
        }
        _ => boolean(node),
    }
}
