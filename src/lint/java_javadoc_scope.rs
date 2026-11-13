use tree_sitter::Node;

use super::java::{
    ANNOTATION_TYPE, CLASS, CONSTRUCTOR, ENUM, ENUM_CONSTANT, INTERFACE, OBJECT_CREATION, PRIVATE, PROTECTED,
    PUBLIC, has_modifier, is_type_declaration,
};

const CODE_BLOCKS: [&str; 6] = [
    "method_declaration",
    CONSTRUCTOR,
    "compact_constructor_declaration",
    "static_initializer",
    "lambda_expression",
    "block",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Public,
    Protected,
    Package,
    Private,
    AnonymousInner,
}

// El `ScopeUtil.getScope` de Checkstyle: el modificador escrito o, si no hay, el implícito del tipo
// que lo encierra (público en una interfaz o una anotación; privado el constructor de un enum).
pub fn declared_scope(node: Node<'_>) -> Scope {
    if has_modifier(node, PUBLIC) {
        Scope::Public
    } else if has_modifier(node, PROTECTED) {
        Scope::Protected
    } else if has_modifier(node, PRIVATE) {
        Scope::Private
    } else {
        implicit_scope(node)
    }
}

fn implicit_scope(node: Node<'_>) -> Scope {
    match nearest_type_block(node).map(|block| block.kind()) {
        Some(ENUM) if node.kind() == ENUM_CONSTANT => Scope::Public,
        Some(ENUM) if node.kind() == CONSTRUCTOR => Scope::Private,
        Some(INTERFACE | ANNOTATION_TYPE) => Scope::Public,
        _ => Scope::Package,
    }
}

fn nearest_type_block(node: Node<'_>) -> Option<Node<'_>> {
    ancestors(node).find(|ancestor| is_type_declaration(*ancestor) || ancestor.kind() == OBJECT_CREATION)
}

// El `getSurroundingScope`: el alcance más cerrado de los tipos que lo encierran, contando al nodo
// si es un tipo; adentro de una clase anónima no hay alcance visible.
pub fn surrounding_scope(node: Node<'_>) -> Scope {
    let mut narrowest = None;
    for current in std::iter::once(node).chain(ancestors(node)) {
        if current.kind() == OBJECT_CREATION {
            return Scope::AnonymousInner;
        }
        if is_type_declaration(current) {
            let scope = declared_scope(current);
            narrowest = Some(narrowest.map_or(scope, |known: Scope| known.max(scope)));
        }
    }
    narrowest.unwrap_or(Scope::Public)
}

// JavadocMethod mira el tipo más cercano que sea clase, interfaz, enum o anotación (un record no
// cuenta y se sigue subiendo) y no entra a las clases anónimas.
pub fn has_javadoc_method_owner(node: Node<'_>) -> bool {
    ancestors(node)
        .find(|ancestor| {
            [CLASS, INTERFACE, ENUM, ANNOTATION_TYPE, OBJECT_CREATION].contains(&ancestor.kind())
        })
        .is_some_and(|owner| owner.kind() != OBJECT_CREATION)
}

pub fn is_in_code_block(node: Node<'_>) -> bool {
    ancestors(node).any(|ancestor| CODE_BLOCKS.contains(&ancestor.kind()))
}

fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}
