use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{
    IDENTIFIER, METHOD, NAME_FIELD, SCOPED_IDENTIFIER, SCOPED_TYPE_IDENTIFIER, TYPE_DECLARATIONS,
    TYPE_IDENTIFIER, text,
};
use crate::syntax::children_with_fields;

const IMPORT: &str = "import_declaration";
const FRAMES: [&str; 7] = [
    "class_body",
    "interface_body",
    "enum_body",
    "annotation_type_body",
    "block",
    "constructor_body",
    "switch_block_statement_group",
];
const GENERIC_TYPE: &str = "generic_type";
const FIELD_ACCESS: &str = "field_access";
const FIELD_FIELD: &str = "field";
const METHOD_INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";
const METHOD_REFERENCE: &str = "method_reference";

#[derive(Debug, Default)]
struct Frame<'source> {
    declared: HashSet<&'source str>,
    referenced: HashSet<&'source str>,
}

impl<'source> Frame<'source> {
    fn finish(mut self) -> HashSet<&'source str> {
        self.referenced.retain(|name| !self.declared.contains(name));
        self.referenced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Declared,
    Referenced,
    Ignored,
}

#[derive(Debug, Clone, Copy)]
enum Visit<'tree> {
    Enter {
        node: Node<'tree>,
        has_type_arguments: bool,
    },
    Leave,
}

type Children<'tree> = [(Option<&'tree str>, Node<'tree>)];

pub fn referenced_names<'source>(root: Node<'_>, source: &'source [u8]) -> HashSet<&'source str> {
    let mut frames = vec![Frame::default()];
    let mut pending = vec![Visit::Enter {
        node: root,
        has_type_arguments: false,
    }];
    while let Some(visit) = pending.pop() {
        let Visit::Enter {
            node,
            has_type_arguments,
        } = visit
        else {
            let finished = frames.pop().map(Frame::finish).unwrap_or_default();
            if let Some(parent) = frames.last_mut() {
                parent.referenced.extend(finished);
            }
            continue;
        };
        if node.kind() == IMPORT {
            continue;
        }
        if FRAMES.contains(&node.kind()) {
            frames.push(Frame::default());
            pending.push(Visit::Leave);
        }
        let children = children_with_fields(node);
        if let Some(frame) = frames.last_mut() {
            let visited = Visited {
                node,
                children: &children,
                has_type_arguments,
            };
            record_names(frame, &visited, source);
        }
        let is_generic = node.kind() == GENERIC_TYPE;
        pending.extend(
            children
                .iter()
                .enumerate()
                .filter(|(_, (_, child))| !is_name(*child))
                .map(|(position, &(_, child))| Visit::Enter {
                    node: child,
                    has_type_arguments: is_generic && position == 0,
                }),
        );
    }
    frames.pop().map(Frame::finish).unwrap_or_default()
}

fn record_names<'source>(frame: &mut Frame<'source>, visited: &Visited<'_, '_>, source: &'source [u8]) {
    let parent = visited.node;
    let shape = Shape {
        parent: parent.kind(),
        has_object: parent.child_by_field_name(OBJECT_FIELD).is_some(),
        has_type_arguments: visited.has_type_arguments,
    };
    let children = visited.children;
    let named: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, (_, child))| child.is_named())
        .map(|(position, _)| position)
        .collect();
    for (index, &position) in named.iter().enumerate() {
        let Some(&(field, child)) = children.get(position) else {
            continue;
        };
        if !is_name(child) {
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|previous| named.get(previous))
            .and_then(|&previous| children.get(previous))
            .map(|(_, node)| node.kind());
        let place = Place {
            field,
            index,
            count: named.len(),
            previous,
        };
        let name = text(child, source);
        match role(&shape, &place) {
            Role::Declared => {
                frame.declared.insert(name);
            }
            Role::Referenced => {
                frame.referenced.insert(name);
            }
            Role::Ignored => {}
        }
    }
}

fn is_name(node: Node<'_>) -> bool {
    [IDENTIFIER, TYPE_IDENTIFIER].contains(&node.kind())
}

struct Visited<'children, 'tree> {
    node: Node<'tree>,
    children: &'children Children<'tree>,
    has_type_arguments: bool,
}

struct Shape<'tree> {
    parent: &'tree str,
    has_object: bool,
    has_type_arguments: bool,
}

struct Place<'tree> {
    field: Option<&'tree str>,
    index: usize,
    count: usize,
    previous: Option<&'tree str>,
}

impl Place<'_> {
    const fn is_first(&self) -> bool {
        self.index == 0
    }

    const fn is_last(&self) -> bool {
        self.index.saturating_add(1) == self.count
    }
}

// Checkstyle cuenta el último nombre de un tipo calificado con argumentos (`Map.Entry<K, V>`) como
// usado, salvo que lo preceda otro nivel (`java.util.List<String>` no cuenta `List`).
fn is_generic_tail(shape: &Shape<'_>, place: &Place<'_>) -> bool {
    place.is_last() && shape.has_type_arguments && place.previous != Some(SCOPED_TYPE_IDENTIFIER)
}

fn role(shape: &Shape<'_>, place: &Place<'_>) -> Role {
    let is_name_field = place.field == Some(NAME_FIELD);
    let ignored = match shape.parent {
        parent if TYPE_DECLARATIONS.contains(&parent) && is_name_field => return Role::Declared,
        METHOD | SCOPED_IDENTIFIER => is_name_field,
        FIELD_ACCESS => place.field == Some(FIELD_FIELD),
        METHOD_INVOCATION => is_name_field && shape.has_object,
        SCOPED_TYPE_IDENTIFIER => !(place.is_first() || is_generic_tail(shape, place)),
        METHOD_REFERENCE => place.is_last() && !place.is_first(),
        _ => false,
    };
    if ignored { Role::Ignored } else { Role::Referenced }
}
