use tree_sitter::Node;

use crate::lint::java::{ANNOTATION, MARKER_ANNOTATION, NAME_FIELD, modifiers, qualified_name, text};
use crate::syntax::named_children;

const VALUE: &str = "Value";
const CONSTRUCTOR_ANNOTATIONS: [&str; 3] = [
    "NoArgsConstructor",
    "RequiredArgsConstructor",
    "AllArgsConstructor",
];
const PRIVATE_ACCESS: &str = "PRIVATE";
const ARGUMENTS_FIELD: &str = "arguments";
const NAME_SEPARATOR: char = '.';

pub fn makes_constructors(class: Node<'_>, source: &[u8]) -> bool {
    modifiers(class).is_some_and(|modifiers| {
        named_children(modifiers)
            .into_iter()
            .filter(|annotation| [ANNOTATION, MARKER_ANNOTATION].contains(&annotation.kind()))
            .any(|annotation| makes_public_constructor(annotation, source))
    })
}

fn makes_public_constructor(annotation: Node<'_>, source: &[u8]) -> bool {
    let Some(name) = annotation.child_by_field_name(NAME_FIELD) else {
        return false;
    };
    let qualified = qualified_name(name, source);
    let simple = qualified.rsplit(NAME_SEPARATOR).next().unwrap_or_default();
    if simple == VALUE {
        return true;
    }
    CONSTRUCTOR_ANNOTATIONS.contains(&simple)
        && annotation
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_none_or(|arguments| !text(arguments, source).contains(PRIVATE_ACCESS))
}
