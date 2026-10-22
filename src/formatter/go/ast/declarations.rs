use super::super::position::Pos;
use super::super::token::Token;
use super::{BasicLit, Block, CommentGroup, Expr, FieldList, FuncType, Ident};

#[derive(Debug, Clone)]
pub struct File {
    pub package: Pos,
    pub name: Ident,
    pub decls: Vec<Decl>,
    pub comments: Vec<CommentGroup>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Gen(GenDecl),
    Func(Box<FuncDecl>),
}

impl Decl {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Gen(decl) => decl.tok_pos,
            Self::Func(decl) => decl.typ.pos(),
        }
    }

    pub fn end(&self) -> Pos {
        match self {
            Self::Gen(decl) => decl.end(),
            Self::Func(decl) => decl.body.as_ref().map_or_else(|| decl.typ.end(), Block::end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenDecl {
    pub tok_pos: Pos,
    pub tok: Token,
    pub lparen: Pos,
    pub specs: Vec<Spec>,
    pub rparen: Pos,
}

impl GenDecl {
    pub fn end(&self) -> Pos {
        if self.rparen.is_valid() {
            return self.rparen.shifted(1);
        }
        self.specs.first().map_or(Pos::NONE, Spec::end)
    }
}

#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub recv: Option<FieldList>,
    pub name: Ident,
    pub typ: FuncType,
    pub body: Option<Block>,
}

#[derive(Debug, Clone)]
pub enum Spec {
    Import(ImportSpec),
    Value(ValueSpec),
    Type(TypeSpec),
}

impl Spec {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Import(spec) => spec.pos(),
            Self::Value(spec) => spec.names.first().map_or(Pos::NONE, |name| name.pos),
            Self::Type(spec) => spec.name.pos,
        }
    }

    pub fn end(&self) -> Pos {
        match self {
            Self::Import(spec) => spec.end(),
            Self::Value(spec) => spec.end(),
            Self::Type(spec) => spec.typ.end(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportSpec {
    pub name: Option<Ident>,
    pub path: BasicLit,
    pub end_pos: Pos,
    pub comment: Option<usize>,
}

impl ImportSpec {
    pub fn pos(&self) -> Pos {
        self.name.as_ref().map_or(self.path.pos, |name| name.pos)
    }

    pub const fn end(&self) -> Pos {
        if self.end_pos.is_valid() {
            self.end_pos
        } else {
            self.path.end
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValueSpec {
    pub names: Vec<Ident>,
    pub typ: Option<Expr>,
    pub values: Vec<Expr>,
    pub comment: Option<usize>,
}

impl ValueSpec {
    pub fn end(&self) -> Pos {
        if let Some(value) = self.values.last() {
            return value.end();
        }
        self.typ
            .as_ref()
            .map_or_else(|| self.names.last().map_or(Pos::NONE, Ident::end), Expr::end)
    }
}

#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub name: Ident,
    pub type_params: Option<FieldList>,
    pub assign: Pos,
    pub typ: Expr,
}
