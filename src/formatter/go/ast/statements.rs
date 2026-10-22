use super::super::position::Pos;
use super::super::token::Token;
use super::{Expr, GenDecl, Ident};

#[derive(Debug, Clone)]
pub struct Block {
    pub lbrace: Pos,
    pub list: Vec<Stmt>,
    pub rbrace: Pos,
}

impl Block {
    pub const fn end(&self) -> Pos {
        self.rbrace.shifted(1)
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Decl(GenDecl),
    Empty(Pos),
    Labeled(Box<LabeledStmt>),
    Expr(Expr),
    Send(Box<SendStmt>),
    IncDec(Box<IncDecStmt>),
    Assign(Box<AssignStmt>),
    Go(Box<CallStmt>),
    Defer(Box<CallStmt>),
    Return(Box<ReturnStmt>),
    Branch(Box<BranchStmt>),
    Block(Box<Block>),
    If(Box<IfStmt>),
    Case(Box<CaseClause>),
    Switch(Box<SwitchStmt>),
    TypeSwitch(Box<TypeSwitchStmt>),
    Comm(Box<CommClause>),
    Select(Box<SelectStmt>),
    For(Box<ForStmt>),
    Range(Box<RangeStmt>),
}

#[derive(Debug, Clone)]
pub struct LabeledStmt {
    pub label: Ident,
    pub colon: Pos,
    pub stmt: Stmt,
}

#[derive(Debug, Clone)]
pub struct SendStmt {
    pub chan: Expr,
    pub arrow: Pos,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct IncDecStmt {
    pub x: Expr,
    pub tok_pos: Pos,
    pub tok: Token,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub lhs: Vec<Expr>,
    pub tok_pos: Pos,
    pub tok: Token,
    pub rhs: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct CallStmt {
    pub keyword: Pos,
    pub call: Expr,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub keyword: Pos,
    pub results: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct BranchStmt {
    pub tok_pos: Pos,
    pub tok: Token,
    pub label: Option<Ident>,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub keyword: Pos,
    pub init: Option<Stmt>,
    pub cond: Expr,
    pub body: Block,
    pub alternative: Option<Stmt>,
}

#[derive(Debug, Clone)]
pub struct CaseClause {
    pub keyword: Pos,
    pub list: Option<Vec<Expr>>,
    pub colon: Pos,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub keyword: Pos,
    pub init: Option<Stmt>,
    pub tag: Option<Expr>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct TypeSwitchStmt {
    pub keyword: Pos,
    pub init: Option<Stmt>,
    pub assign: Stmt,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct CommClause {
    pub keyword: Pos,
    pub comm: Option<Stmt>,
    pub colon: Pos,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct SelectStmt {
    pub keyword: Pos,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub keyword: Pos,
    pub init: Option<Stmt>,
    pub cond: Option<Expr>,
    pub post: Option<Stmt>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct RangeStmt {
    pub keyword: Pos,
    pub key: Option<Expr>,
    pub value: Option<Expr>,
    pub tok_pos: Pos,
    pub tok: Token,
    pub x: Expr,
    pub body: Block,
}

impl Stmt {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Decl(decl) => decl.tok_pos,
            Self::Empty(pos) => *pos,
            Self::Labeled(labeled) => labeled.label.pos,
            Self::Expr(expr) => expr.pos(),
            Self::Send(send) => send.chan.pos(),
            Self::IncDec(statement) => statement.x.pos(),
            Self::Assign(assign) => assign.lhs.first().map_or(Pos::NONE, Expr::pos),
            Self::Go(statement) | Self::Defer(statement) => statement.keyword,
            Self::Return(statement) => statement.keyword,
            Self::Branch(branch) => branch.tok_pos,
            Self::Block(block) => block.lbrace,
            Self::If(statement) => statement.keyword,
            Self::Case(clause) => clause.keyword,
            Self::Switch(statement) => statement.keyword,
            Self::TypeSwitch(statement) => statement.keyword,
            Self::Comm(clause) => clause.keyword,
            Self::Select(statement) => statement.keyword,
            Self::For(statement) => statement.keyword,
            Self::Range(statement) => statement.keyword,
        }
    }
}
