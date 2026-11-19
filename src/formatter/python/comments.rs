pub mod format;
pub mod placement;
pub mod suppression;
pub mod visitor;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use super::ast::{Ast, NodeId, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePosition {
    EndOfLine,
    OwnLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceComment {
    pub span: Span,
    pub line_position: LinePosition,
    pub formatted: Cell<bool>,
}

impl SourceComment {
    pub const fn new(span: Span, line_position: LinePosition) -> Self {
        Self {
            span,
            line_position,
            formatted: Cell::new(false),
        }
    }

    pub const fn is_unformatted(&self) -> bool {
        !self.formatted.get()
    }

    pub fn mark_formatted(&self) {
        self.formatted.set(true);
    }

    pub fn mark_unformatted(&self) {
        self.formatted.set(false);
    }

    pub fn is_own_line(&self) -> bool {
        self.line_position == LinePosition::OwnLine
    }
}

#[derive(Debug, Default)]
pub struct NodeComments {
    pub leading: Vec<SourceComment>,
    pub dangling: Vec<SourceComment>,
    pub trailing: Vec<SourceComment>,
}

// Los comentarios del archivo repartidos entre los nodos, como `Comments` de ruff: cada uno va
// antes de su nodo (leading), después (trailing) o suelto adentro (dangling).
#[derive(Debug, Default)]
pub struct Comments {
    by_node: HashMap<NodeId, NodeComments>,
    containing: HashSet<NodeId>,
}

impl Comments {
    pub fn of(&self, node: NodeId) -> Option<&NodeComments> {
        self.by_node.get(&node)
    }

    pub fn leading(&self, node: NodeId) -> &[SourceComment] {
        self.of(node).map_or(&[], |comments| &comments.leading)
    }

    pub fn dangling(&self, node: NodeId) -> &[SourceComment] {
        self.of(node).map_or(&[], |comments| &comments.dangling)
    }

    pub fn trailing(&self, node: NodeId) -> &[SourceComment] {
        self.of(node).map_or(&[], |comments| &comments.trailing)
    }

    pub fn has_leading(&self, node: NodeId) -> bool {
        !self.leading(node).is_empty()
    }

    pub fn has_dangling(&self, node: NodeId) -> bool {
        !self.dangling(node).is_empty()
    }

    pub fn has_trailing(&self, node: NodeId) -> bool {
        !self.trailing(node).is_empty()
    }

    pub fn has_trailing_own_line(&self, node: NodeId) -> bool {
        self.trailing(node).iter().any(SourceComment::is_own_line)
    }

    pub fn has(&self, node: NodeId) -> bool {
        self.by_node.contains_key(&node)
    }

    // Si el nodo o alguno de sus descendientes tiene comentarios.
    pub fn contains_comments(&self, node: NodeId) -> bool {
        self.containing.contains(&node)
    }

    pub fn first_unformatted(&self) -> Option<Span> {
        self.by_node
            .values()
            .flat_map(|comments| {
                comments
                    .leading
                    .iter()
                    .chain(&comments.dangling)
                    .chain(&comments.trailing)
            })
            .filter(|comment| comment.is_unformatted())
            .map(|comment| comment.span)
            .min_by_key(|span| span.start)
    }

    pub fn entry(&mut self, ast: &Ast, node: NodeId) -> &mut NodeComments {
        let mut ancestor = Some(node);
        while let Some(current) = ancestor {
            if !self.containing.insert(current) {
                break;
            }
            ancestor = ast.parent(current);
        }
        self.by_node.entry(node).or_default()
    }
}
