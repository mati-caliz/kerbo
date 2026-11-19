use std::iter::Peekable;
use std::slice::Iter;

use super::placement::{Placer, place_comment};
use super::{Comments, LinePosition, SourceComment};
use crate::formatter::python::ast::{NodeId, Span};
use crate::formatter::python::context::Unsupported;
use crate::formatter::python::trivia::{is_line_break, is_python_whitespace};

// Un comentario con lo que lo rodea, como `DecoratedComment` de ruff: el nodo más chico que lo
// contiene, y los hijos de ese nodo que tiene antes y después.
#[derive(Debug, Clone, Copy)]
pub struct DecoratedComment {
    pub enclosing: NodeId,
    pub enclosing_parent: Option<NodeId>,
    pub preceding: Option<NodeId>,
    pub following: Option<NodeId>,
    pub line_position: LinePosition,
    pub span: Span,
}

impl DecoratedComment {
    pub fn is_own_line(&self) -> bool {
        self.line_position == LinePosition::OwnLine
    }

    pub fn is_end_of_line(&self) -> bool {
        self.line_position == LinePosition::EndOfLine
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Leading(NodeId),
    Trailing(NodeId),
    Dangling(NodeId),
    Default,
    Unsupported(&'static str),
}

impl Placement {
    pub fn or_else(self, next: impl FnOnce() -> Self) -> Self {
        if self == Self::Default { next() } else { self }
    }
}

fn line_position_of(source: &str, comment: Span) -> LinePosition {
    let before = source.get(..comment.start).unwrap_or_default();
    let has_content_before = before
        .chars()
        .rev()
        .take_while(|character| !is_line_break(*character))
        .any(|character| !is_python_whitespace(character));
    if has_content_before {
        LinePosition::EndOfLine
    } else {
        LinePosition::OwnLine
    }
}

// Sin una regla propia, el comentario al final de una línea es de lo que tiene antes, y el que
// está en su propia línea, de lo que tiene después.
fn default_placement(comment: &DecoratedComment) -> Placement {
    let dangling = Placement::Dangling(comment.enclosing);
    match comment.line_position {
        LinePosition::EndOfLine => comment
            .preceding
            .map(Placement::Trailing)
            .or_else(|| comment.following.map(Placement::Leading))
            .unwrap_or(dangling),
        LinePosition::OwnLine => comment
            .following
            .map(Placement::Leading)
            .or_else(|| comment.preceding.map(Placement::Trailing))
            .unwrap_or(dangling),
    }
}

#[derive(Debug)]
struct Visitor<'placer, 'source> {
    placer: &'placer Placer<'source>,
    comments: Comments,
    unsupported: Option<Unsupported>,
    parents: Vec<NodeId>,
    preceding: Option<NodeId>,
    pending: Peekable<Iter<'source, Span>>,
}

impl Visitor<'_, '_> {
    fn decorated(&self, comment: Span, enclosing: NodeId) -> DecoratedComment {
        DecoratedComment {
            enclosing,
            enclosing_parent: None,
            preceding: self.preceding,
            following: None,
            line_position: line_position_of(self.placer.source, comment),
            span: comment,
        }
    }

    fn can_skip(&mut self, node_end: usize) -> bool {
        self.pending
            .peek()
            .is_none_or(|comment| comment.start >= node_end)
    }

    fn enter(&mut self, node: NodeId) -> bool {
        let span = self.placer.ast.span(node);
        let enclosing = self.parents.last().copied().unwrap_or(node);
        while let Some(comment) = self.pending.next_if(|comment| comment.end <= span.start) {
            let decorated = DecoratedComment {
                enclosing_parent: self.parents.iter().rev().nth(1).copied(),
                following: Some(node),
                ..self.decorated(*comment, enclosing)
            };
            self.push(&decorated);
        }
        self.preceding = None;
        self.parents.push(node);
        !self.can_skip(span.end)
    }

    fn leave(&mut self, node: NodeId) {
        self.parents.pop();
        let node_end = self.placer.ast.end(node);
        let is_root = self.parents.is_empty();
        while let Some(comment) = self
            .pending
            .next_if(|comment| is_root || comment.start < node_end)
        {
            let decorated = DecoratedComment {
                enclosing_parent: self.parents.last().copied(),
                ..self.decorated(*comment, node)
            };
            self.push(&decorated);
        }
        self.preceding = Some(node);
    }

    fn visit(&mut self, node: NodeId) {
        if self.enter(node) {
            for child in self.placer.ast.children(node) {
                self.visit(child);
            }
        }
        self.leave(node);
    }

    fn push(&mut self, comment: &DecoratedComment) {
        let placement = place_comment(self.placer, comment).or_else(|| default_placement(comment));
        let source_comment = SourceComment::new(comment.span, comment.line_position);
        let ast = self.placer.ast;
        match placement {
            Placement::Leading(node) => self.comments.entry(ast, node).leading.push(source_comment),
            Placement::Trailing(node) => self.comments.entry(ast, node).trailing.push(source_comment),
            Placement::Dangling(node) => self.comments.entry(ast, node).dangling.push(source_comment),
            Placement::Default => {}
            Placement::Unsupported(syntax) => {
                self.unsupported.get_or_insert(Unsupported {
                    offset: comment.span.start,
                    syntax,
                });
            }
        }
    }
}

// Reparte los comentarios del archivo entre los nodos, recorriendo el árbol en el orden del
// fuente como `CommentsVisitor` de ruff.
pub fn collect(placer: &Placer<'_>, root: NodeId) -> Result<Comments, Unsupported> {
    let mut visitor = Visitor {
        placer,
        comments: Comments::default(),
        unsupported: None,
        parents: Vec::new(),
        preceding: None,
        pending: placer.comment_ranges.iter().peekable(),
    };
    visitor.visit(root);
    match visitor.unsupported {
        Some(unsupported) => Err(unsupported),
        None => Ok(visitor.comments),
    }
}
