mod binary;
mod declarations;
mod expressions;
mod fields;
mod lists;
mod numbers;
mod signatures;
mod sizes;
mod slices;
mod statements;
mod walk;

use std::mem::Discriminant;

use super::ast::{Expr, Stmt};
use super::position::Pos;

pub use declarations::file;

#[derive(Debug, Clone, Copy)]
pub enum Node<'tree> {
    Expr(&'tree Expr),
    Stmt(&'tree Stmt),
}

// go/printer guarda el tamaño de cada nodo por identidad; acá la identidad es el tipo de nodo y su
// tramo del fuente, que es lo mismo para el AST que sale del parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKey {
    Expr(Discriminant<Expr>, Pos, Pos),
    Stmt(Discriminant<Stmt>, Pos),
}

impl Node<'_> {
    pub fn key(self) -> NodeKey {
        match self {
            Self::Expr(expr) => NodeKey::Expr(std::mem::discriminant(expr), expr.pos(), expr.end()),
            Self::Stmt(stmt) => NodeKey::Stmt(std::mem::discriminant(stmt), stmt.pos()),
        }
    }
}
