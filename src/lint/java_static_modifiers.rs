use tree_sitter::Node;

use super::java::{ABSTRACT, FINAL, METHOD, NAME_FIELD, STATIC, has_modifier, modifier_tokens, text};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::descendants;

const FIELD: &str = "field_declaration";
const TRANSIENT: &str = "transient";
const DECLARATOR: &str = "declarator";
const INTERFACE_BODIES: [&str; 2] = ["interface_body", "annotation_type_body"];

// `FinalStaticMethod` de IntelliJ, y la parte de `UnnecessaryModifier` que marca `transient` en un
// campo `static`: dos modificadores que no cambian nada y que ni Checkstyle ni PMD reportan.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    descendants(root)
        .into_iter()
        .flat_map(|node| match node.kind() {
            METHOD if is_final_static_method(node) => redundant(
                modifier_tokens(node, FINAL),
                JavaRule::FinalStaticMethod,
                &declared_name(node, source),
                source,
            ),
            FIELD if has_modifier(node, STATIC) => redundant(
                modifier_tokens(node, TRANSIENT),
                JavaRule::TransientStaticField,
                &field_names(node, source),
                source,
            ),
            _ => Vec::new(),
        })
        .collect()
}

fn is_final_static_method(method: Node<'_>) -> bool {
    has_modifier(method, STATIC)
        && has_modifier(method, FINAL)
        && !has_modifier(method, ABSTRACT)
        && !method
            .parent()
            .is_some_and(|parent| INTERFACE_BODIES.contains(&parent.kind()))
}

fn redundant(modifiers: Vec<Node<'_>>, rule: JavaRule, subject: &str, source: &[u8]) -> Vec<LintFinding> {
    modifiers
        .into_iter()
        .map(|modifier| {
            finding(
                LintKind::Java(rule),
                modifier,
                subject,
                Some(Fix::remove(modifier, source)),
            )
        })
        .collect()
}

fn declared_name(declaration: Node<'_>, source: &[u8]) -> String {
    declaration
        .child_by_field_name(NAME_FIELD)
        .map_or_else(String::new, |name| text(name, source).to_owned())
}

fn field_names(field: Node<'_>, source: &[u8]) -> String {
    let mut cursor = field.walk();
    field
        .children_by_field_name(DECLARATOR, &mut cursor)
        .map(|declarator| declared_name(declarator, source))
        .collect::<Vec<_>>()
        .join(", ")
}
