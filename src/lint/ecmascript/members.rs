use tree_sitter::Node;

use super::nodes::{
    INDEX_FIELD, MEMBER, NUMBER, PROPERTY_FIELD, STRING, SUBSCRIPT, TEMPLATE_STRING, text,
    without_parentheses,
};
use super::strings::{has_substitutions, numeric_text, static_string};

const PROPERTY_IDENTIFIER: &str = "property_identifier";
const COMPUTED_KEY: &str = "computed_property_name";
const KEYWORD_LITERALS: &[&str] = &["null", "true", "false", "regex"];

// `getStaticPropertyName` de un acceso a miembro: `a.b`, `a['b']`, `a[1]`, `` a[`b`] ``.
pub fn static_property_name(member: Node<'_>, source: &[u8]) -> Option<String> {
    match member.kind() {
        MEMBER => member
            .child_by_field_name(PROPERTY_FIELD)
            .filter(|property| property.kind() == PROPERTY_IDENTIFIER)
            .map(|property| text(property, source).to_owned()),
        SUBSCRIPT => member
            .child_by_field_name(INDEX_FIELD)
            .and_then(|index| static_string_value(without_parentheses(index), source)),
        _ => None,
    }
}

// El nombre estático de la clave de una propiedad de objeto o de patrón.
pub fn static_key_name(key: Node<'_>, source: &[u8]) -> Option<String> {
    match key.kind() {
        COMPUTED_KEY => key
            .named_child(0)
            .and_then(|expression| static_string_value(without_parentheses(expression), source)),
        PROPERTY_IDENTIFIER => Some(text(key, source).to_owned()),
        _ => static_string_value(key, source),
    }
}

// `getStaticStringValue`: el valor de un literal o de un template sin sustituciones, como texto.
pub fn static_string_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        STRING => static_string(node, source),
        TEMPLATE_STRING if !has_substitutions(node) => static_string(node, source),
        NUMBER => numeric_text(text(node, source)),
        kind if KEYWORD_LITERALS.contains(&kind) => Some(text(node, source).to_owned()),
        _ => None,
    }
}
