use tree_sitter::Node;

use crate::profile::{ParameterShape, Profile};

const NAME_FIELD: &str = "name";
const IDENTIFIER_KIND: &str = "identifier";
const MODIFIERS_KIND: &str = "modifiers";

pub(super) fn parameter_count(unit: Node<'_>, profile: &Profile, source: &[u8]) -> Option<usize> {
    let shape = &profile.parameters;
    if !shape.checked_units.contains(&unit.kind()) || has_exempting_annotation(unit, shape, source) {
        return None;
    }
    if shape
        .single_field
        .and_then(|field| unit.child_by_field_name(field))
        .is_some()
    {
        return Some(1);
    }
    let Some(list) = unit.child_by_field_name(shape.list_field) else {
        return Some(0);
    };
    let mut cursor = list.walk();
    let declarations = list
        .named_children(&mut cursor)
        .filter(|&declaration| is_counted(declaration, shape, source));
    let count = if shape.names_per_declaration {
        declarations
            .map(|declaration| declared_names(declaration).max(1))
            .sum()
    } else {
        declarations.count()
    };
    Some(count)
}

fn is_counted(declaration: Node<'_>, shape: &ParameterShape, source: &[u8]) -> bool {
    if !shape.counted.contains(&declaration.kind()) {
        return false;
    }
    let first_inner = declaration.named_child(0);
    if first_inner.is_some_and(|inner| shape.excluded_inner.contains(&inner.kind())) {
        return false;
    }
    let name_node = if declaration.kind() == IDENTIFIER_KIND {
        Some(declaration)
    } else {
        first_inner
    };
    !name_node
        .filter(|name| name.kind() == IDENTIFIER_KIND)
        .and_then(|name| name.utf8_text(source).ok())
        .is_some_and(|name| shape.excluded_names.contains(&name))
}

fn has_exempting_annotation(unit: Node<'_>, shape: &ParameterShape, source: &[u8]) -> bool {
    let Some(annotation_name) = shape.exempting_annotation else {
        return false;
    };
    let mut cursor = unit.walk();
    let modifiers = unit
        .children(&mut cursor)
        .find(|child| child.kind() == MODIFIERS_KIND);
    modifiers.is_some_and(|modifiers| annotates_with(modifiers, annotation_name, source))
}

fn annotates_with(modifiers: Node<'_>, annotation_name: &str, source: &[u8]) -> bool {
    let mut cursor = modifiers.walk();
    let mut annotations = modifiers.named_children(&mut cursor);
    annotations.any(|annotation| {
        annotation
            .child_by_field_name(NAME_FIELD)
            .and_then(|name| name.utf8_text(source).ok())
            .is_some_and(|name| name == annotation_name)
    })
}

fn declared_names(declaration: Node<'_>) -> usize {
    let mut cursor = declaration.walk();
    declaration
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .count()
}
