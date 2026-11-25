mod bodies;
pub mod branches;
mod expressions;
mod others;
mod statements;

use std::collections::HashSet;

use super::visitor::{DecoratedComment, Placement};
use crate::formatter::python::ast::{Ast, NodeId, Span};
use crate::formatter::python::trivia::{Token, TokenKind, Tokenizer};

// Lo que necesitan las reglas para ubicar un comentario: el árbol, el fuente para mirar los
// tokens que hay entre dos nodos, y qué expresiones están entre paréntesis.
#[derive(Debug, Clone, Copy)]
pub struct Placer<'source> {
    pub ast: &'source Ast,
    pub source: &'source str,
    pub comment_ranges: &'source [Span],
    pub parenthesized: &'source HashSet<Span>,
}

impl<'source> Placer<'source> {
    fn tokens(&self, start: usize, end: usize) -> impl Iterator<Item = Token> + 'source {
        Tokenizer::new(self.source, Span::new(start, end)).skip_trivia()
    }

    fn has_token(&self, (start, end): (usize, usize), kind: TokenKind) -> bool {
        self.tokens(start, end).any(|token| token.kind == kind)
    }

    // `find_only_token_in_range` de ruff: el único token entre dos nodos, sin los paréntesis que
    // cierran al primero.
    fn token_after_closing_parentheses(&self, start: usize, end: usize) -> Option<Token> {
        self.tokens(start, end)
            .find(|token| token.kind != TokenKind::RParen)
    }

    fn start(&self, node: NodeId) -> usize {
        self.ast.start(node)
    }

    fn end(&self, node: NodeId) -> usize {
        self.ast.end(node)
    }
}

const fn opens_a_definition_or_binding(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::As | TokenKind::Def | TokenKind::Class)
}

fn has_parenthesis_between(
    placer: &Placer<'_>,
    (start, end): (usize, usize),
    parenthesis: TokenKind,
) -> bool {
    placer
        .tokens(start, end)
        .take_while(|token| !opens_a_definition_or_binding(token.kind))
        .any(|token| token.kind == parenthesis)
}

// Un comentario adentro de unos paréntesis pero afuera de la expresión que encierran: si entre
// el nodo anterior y el comentario se abre un paréntesis, es del nodo que sigue; si entre el
// comentario y el nodo que sigue se cierra uno, es del anterior.
fn handle_parenthesized_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (Some(preceding), Some(following)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let before = (placer.end(preceding), comment.span.start);
    if has_parenthesis_between(placer, before, TokenKind::LParen) {
        return Placement::Leading(following);
    }
    let after = (comment.span.end, placer.start(following));
    if has_parenthesis_between(placer, after, TokenKind::RParen) {
        return Placement::Trailing(preceding);
    }
    Placement::Default
}

// Un comentario al final de la línea del paréntesis, corchete o llave que abre queda suelto en
// el nodo, para que siga en esa línea.
fn handle_bracketed_end_of_line_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    if comment.is_own_line() {
        return Placement::Default;
    }
    let mut tokens = placer.tokens(placer.start(comment.enclosing), comment.span.start);
    if tokens.next().is_some() && tokens.next().is_none() {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_enclosed_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    expressions::handle(placer, comment)
        .or_else(|| statements::handle(placer, comment))
        .or_else(|| others::handle(placer, comment))
}

// `place_comment` de ruff: las reglas que corrigen lo que la ubicación por defecto hace mal.
pub fn place_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    handle_parenthesized_comment(placer, comment)
        .or_else(|| bodies::handle_end_of_line_comment_around_body(placer, comment))
        .or_else(|| bodies::handle_own_line_comment_around_body(placer, comment))
        .or_else(|| handle_enclosed_comment(placer, comment))
}
