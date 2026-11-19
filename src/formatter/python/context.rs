use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

use super::ast::{Ast, NodeId, Span};
use super::comments::Comments;
use crate::formatter::document::{Builder, GroupId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelPosition {
    Last,
    Other,
}

// Dónde está lo que se formatea: decide cuántas líneas en blanco se conservan y si una expresión
// ya tiene paréntesis que le permiten partirse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeLevel {
    TopLevel(TopLevelPosition),
    CompoundStatement,
    Expression(Option<GroupId>),
    ParenthesizedExpression,
}

impl NodeLevel {
    pub const fn is_parenthesized(self) -> bool {
        matches!(self, Self::Expression(Some(_)) | Self::ParenthesizedExpression)
    }

    pub const fn is_top_level(self) -> bool {
        matches!(self, Self::TopLevel(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    Single,
    Double,
}

impl Quote {
    pub const fn as_char(self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"',
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::Single => Self::Double,
            Self::Double => Self::Single,
        }
    }
}

// Sintaxis que el formateador todavía no escribe: el archivo queda como está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported {
    pub offset: usize,
    pub syntax: &'static str,
}

pub type Formatted<T = ()> = Result<T, Unsupported>;

// El `PyFormatter` de ruff con su contexto: el documento que se va armando, el fuente, el AST y
// los comentarios ya repartidos entre los nodos.
#[derive(Debug)]
pub struct Formatter<'source> {
    pub builder: Builder,
    pub source: &'source str,
    pub ast: &'source Ast,
    pub comments: &'source Comments,
    pub parenthesized: &'source HashSet<Span>,
    pub comment_ranges: &'source [Span],
    pub node_level: NodeLevel,
    pub indent_level: u16,
    pub docstring_quote: Option<Quote>,
}

impl<'source> Formatter<'source> {
    pub fn text_of(&self, span: Span) -> &'source str {
        self.source.get(span.start..span.end).unwrap_or_default()
    }

    pub fn node_text(&self, node: NodeId) -> &'source str {
        self.text_of(self.ast.span(node))
    }

    pub fn source_text(&mut self, span: Span) {
        let text = self.text_of(span);
        self.builder.text(text);
    }

    pub fn node_source(&mut self, node: NodeId) {
        self.source_text(self.ast.span(node));
    }

    pub fn is_expression_parenthesized(&self, expression: NodeId) -> bool {
        self.parenthesized.contains(&self.ast.span(expression))
    }

    pub const fn enter(&mut self, level: NodeLevel) -> NodeLevel {
        std::mem::replace(&mut self.node_level, level)
    }

    pub fn unsupported(&self, node: NodeId, syntax: &'static str) -> Unsupported {
        Unsupported {
            offset: self.ast.start(node),
            syntax,
        }
    }
}

impl Deref for Formatter<'_> {
    type Target = Builder;

    fn deref(&self) -> &Builder {
        &self.builder
    }
}

impl DerefMut for Formatter<'_> {
    fn deref_mut(&mut self) -> &mut Builder {
        &mut self.builder
    }
}
