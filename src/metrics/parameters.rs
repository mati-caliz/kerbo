use tree_sitter::Node;

use crate::profile::{ParameterShape, Profile};

const NAME_FIELD: &str = "name";
const IDENTIFIER_KIND: &str = "identifier";
const MODIFIERS_KIND: &str = "modifiers";
const DECORATED_KIND: &str = "decorated_definition";
const DECORATOR_KIND: &str = "decorator";
const DUMMY_PREFIX: char = '_';

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
    let has_receiver = is_method(unit, shape) && !has_static_decorator(unit, shape, source);
    let mut cursor = list.walk();
    let declarations = list
        .named_children(&mut cursor)
        .filter(|declaration| shape.counted.contains(&declaration.kind()))
        .enumerate()
        .filter(|&(position, declaration)| {
            !(has_receiver && position == 0) && is_counted(declaration, shape, source)
        })
        .map(|(_, declaration)| declaration);
    let count = if shape.names_per_declaration {
        declarations.map(declared_names).sum()
    } else {
        declarations.count()
    };
    Some(count)
}

fn is_counted(declaration: Node<'_>, shape: &ParameterShape, source: &[u8]) -> bool {
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
        .is_some_and(|name| shape.excludes_dummy_names && is_dummy_name(name))
}

fn is_dummy_name(name: &str) -> bool {
    let without_underscores = name.trim_start_matches(DUMMY_PREFIX);
    let has_prefix = without_underscores.len() < name.len();
    has_prefix
        && (without_underscores.is_empty() || name.ends_with(|last: char| last.is_ascii_alphanumeric()))
}

fn has_static_decorator(unit: Node<'_>, shape: &ParameterShape, source: &[u8]) -> bool {
    let decorated = unit.parent().filter(|parent| parent.kind() == DECORATED_KIND);
    shape
        .static_decorator
        .is_some_and(|name| decorated.is_some_and(|decorated| decorates_with(decorated, name, source)))
}

fn has_exempting_annotation(unit: Node<'_>, shape: &ParameterShape, source: &[u8]) -> bool {
    let mut cursor = unit.walk();
    let modifiers = unit
        .children(&mut cursor)
        .find(|child| child.kind() == MODIFIERS_KIND);
    let decorated = unit.parent().filter(|parent| parent.kind() == DECORATED_KIND);
    shape.exempting_annotations.iter().any(|&annotation_name| {
        modifiers.is_some_and(|modifiers| annotates_with(modifiers, annotation_name, source))
            || decorated.is_some_and(|decorated| decorates_with(decorated, annotation_name, source))
    })
}

fn decorates_with(decorated: Node<'_>, annotation_name: &str, source: &[u8]) -> bool {
    let mut cursor = decorated.walk();
    let mut decorators = decorated
        .named_children(&mut cursor)
        .filter(|child| child.kind() == DECORATOR_KIND);
    decorators.any(|decorator| {
        decorator.utf8_text(source).is_ok_and(|text| {
            text.trim_start_matches('@')
                .rsplit('.')
                .next()
                .is_some_and(|name| name.trim() == annotation_name)
        })
    })
}

fn is_method(unit: Node<'_>, shape: &ParameterShape) -> bool {
    let Some(container) = shape.method_container else {
        return false;
    };
    std::iter::successors(unit.parent(), Node::parent)
        .find(|node| node.kind() == container || shape.checked_units.contains(&node.kind()))
        .is_some_and(|scope| scope.kind() == container)
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

#[cfg(test)]
mod tests {
    use tree_sitter::Parser;

    use super::is_dummy_name;
    use crate::analysis::analyze;
    use crate::language::Language;
    use crate::limits::Limits;

    #[test]
    fn methods_defined_under_a_condition_still_skip_self_like_ruff() {
        let source = "class A:\n    if flag:\n        def kill(self, signal):\n            pass\n\ndef outer(self):\n    def inner(self):\n        pass\n";
        let mut parser = Parser::new();
        parser
            .set_language(&Language::Python.grammar())
            .expect("la gramática carga");
        let counts: Vec<(String, Option<usize>)> = analyze(
            &mut parser,
            Language::Python,
            source.as_bytes(),
            Limits::default(),
        )
        .expect("el archivo se parsea")
        .functions
        .into_iter()
        .map(|function| (function.name, function.parameters))
        .collect();
        assert_eq!(
            counts,
            [
                ("kill".to_owned(), Some(1)),
                ("outer".to_owned(), Some(1)),
                ("inner".to_owned(), Some(1))
            ],
            "un def bajo un if de la clase es método; adentro de una función, no"
        );
    }

    #[test]
    fn recognizes_ruff_dummy_names() {
        assert!(is_dummy_name("_"));
        assert!(is_dummy_name("__"));
        assert!(is_dummy_name("_browser"));
        assert!(is_dummy_name("_api_key2"));
        assert!(!is_dummy_name("browser"));
        assert!(!is_dummy_name("__dunder__"));
    }
}
