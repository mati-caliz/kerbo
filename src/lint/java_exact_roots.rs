use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_receivers::simple_type_name;
use crate::syntax::{descendants, named_children};

const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const IDENTIFIER: &str = "identifier";
const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const ASSIGNMENT: &str = "assignment_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const CLASS_BODY: &str = "class_body";
const METHODS: &[&str] = &["method_declaration", "constructor_declaration"];
const OBJECT_FIELD: &str = "object";
const VALUE_FIELD: &str = "value";
const TYPE_FIELD: &str = "type";
const LEFT_FIELD: &str = "left";
const ARGUMENTS_FIELD: &str = "arguments";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Con qué se creó el valor de la variable donde empieza una cadena, si se sabe mejor que su tipo
// declarado: un `new Tipo(...)` o una fábrica estática (`HashBiMap.create(n)`), que el índice
// resuelve por lo que devuelve. IntelliJ busca el método en ese tipo: `bimap.get(...)` sobre un
// `BiMap` creado con `HashBiMap.create` usa el `@Nullable V get` de `HashBiMap`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactRoot {
    Construction(String),
    Factory {
        class: String,
        method: String,
        arity: usize,
    },
}

// La raíz de la cadena que termina en la llamada, si es una local que se inicializa una sola vez con
// un `new` o una fábrica estática y no se reasigna.
pub fn exact_root(invocation: Node<'_>, source: &[u8]) -> Option<ExactRoot> {
    let mut current = invocation;
    while current.kind() == INVOCATION {
        current = current.child_by_field_name(OBJECT_FIELD)?;
    }
    if current.kind() != IDENTIFIER {
        return None;
    }
    let name = text(current, source);
    let method =
        std::iter::successors(current.parent(), Node::parent).find(|node| METHODS.contains(&node.kind()))?;
    let nodes = descendants(method);
    let reassigned = nodes.iter().any(|node| {
        node.kind() == ASSIGNMENT
            && node
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.kind() == IDENTIFIER && text(left, source) == name)
    });
    if reassigned {
        return None;
    }
    let mut declarators = nodes
        .iter()
        .filter(|node| node.kind() == LOCAL)
        .flat_map(|declaration| named_children(*declaration))
        .filter(|declarator| declarator.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| {
            declarator
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
        });
    let (Some(declarator), None) = (declarators.next(), declarators.next()) else {
        return None;
    };
    created_by(declarator.child_by_field_name(VALUE_FIELD)?, source)
}

fn created_by(value: Node<'_>, source: &[u8]) -> Option<ExactRoot> {
    let value = if value.kind() == PARENTHESIZED {
        named_children(value)
            .into_iter()
            .find(|child| !COMMENTS.contains(&child.kind()))?
    } else {
        value
    };
    match value.kind() {
        CREATION
            if !named_children(value)
                .iter()
                .any(|child| child.kind() == CLASS_BODY) =>
        {
            let written = value.child_by_field_name(TYPE_FIELD)?;
            Some(ExactRoot::Construction(simple_type_name(text(written, source))))
        }
        INVOCATION => {
            let class = value
                .child_by_field_name(OBJECT_FIELD)
                .filter(|object| object.kind() == IDENTIFIER)
                .map(|object| text(object, source))
                .filter(|written| written.starts_with(|first: char| first.is_ascii_uppercase()))?;
            let arity = value
                .child_by_field_name(ARGUMENTS_FIELD)
                .map(named_children)
                .unwrap_or_default()
                .iter()
                .filter(|argument| !COMMENTS.contains(&argument.kind()))
                .count();
            Some(ExactRoot::Factory {
                class: class.to_owned(),
                method: text(value.child_by_field_name(NAME_FIELD)?, source).to_owned(),
                arity,
            })
        }
        _ => None,
    }
}
