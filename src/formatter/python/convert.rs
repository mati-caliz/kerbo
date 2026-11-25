mod calls;
mod collections;
mod compound;
mod expressions;
mod interpolated;
mod parameters;
mod pattern_classes;
mod patterns;
mod statements;
mod strings;
mod type_params;
mod types;

use tree_sitter::Node;

use std::collections::HashSet;

use super::ast::{Ast, Data, Expr, NodeId, Other, Span, Stmt};

// Si algo todavía no se convierte, el error es su nodo de tree-sitter.
pub type Outcome<'tree, T = NodeId> = Result<T, Node<'tree>>;

#[derive(Debug)]
pub struct Converter<'source> {
    pub source: &'source str,
    pub ast: Ast,
    pub parenthesized: HashSet<Span>,
}

impl<'source> Converter<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            ast: Ast::default(),
            parenthesized: HashSet::new(),
        }
    }

    pub fn text(&self, node: Node<'_>) -> &str {
        self.source.get(node.byte_range()).unwrap_or_default()
    }

    pub fn statement(&mut self, node: Node<'_>, statement: Stmt) -> NodeId {
        self.ast.add(span(node), Data::Stmt(statement))
    }

    pub fn expression(&mut self, node: Node<'_>, expression: Expr) -> NodeId {
        self.ast.add(span(node), Data::Expr(expression))
    }

    pub fn other(&mut self, node: Node<'_>, other: Other) -> NodeId {
        self.ast.add(span(node), Data::Other(other))
    }
}

pub fn span(node: Node<'_>) -> Span {
    Span::new(node.start_byte(), node.end_byte())
}

// Los hijos sin los comentarios ni las barras de continuación, que tree-sitter deja en cualquier
// lugar del árbol.
pub fn parts(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| !child.is_extra())
        .collect()
}

pub fn named(node: Node<'_>) -> Vec<Node<'_>> {
    parts(node).into_iter().filter(Node::is_named).collect()
}

pub fn token<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    parts(node)
        .into_iter()
        .find(|child| !child.is_named() && child.kind() == kind)
}

pub fn has_token(node: Node<'_>, kind: &str) -> bool {
    token(node, kind).is_some()
}

pub fn field<'tree>(node: Node<'tree>, name: &str) -> Outcome<'tree, Node<'tree>> {
    node.child_by_field_name(name).ok_or(node)
}

pub fn only_named(node: Node<'_>) -> Outcome<'_, Node<'_>> {
    match named(node).as_slice() {
        [only] => Ok(*only),
        _ => Err(node),
    }
}

// Convierte el árbol de tree-sitter en el AST de ruff. Devuelve el módulo.
pub fn module<'tree>(converter: &mut Converter<'_>, root: Node<'tree>) -> Outcome<'tree> {
    let body = statements::block(converter, root)?;
    let whole_file = Span::new(0, converter.source.len());
    let module = converter.ast.add(whole_file, Data::Other(Other::Module { body }));
    converter.ast.end_bodies_at_their_last_statement();
    converter.ast.link_parents(module);
    Ok(module)
}
