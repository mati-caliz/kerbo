mod convert;
mod declarations;
mod statements;
mod walk;

use tree_sitter::Point;

pub use convert::file;
pub use walk::{Visitor, walk_expr, walk_field_list, walk_func_type, walk_stmt, walk_stmts};

pub type IdentId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub offset: usize,
    pub start: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Address,
    Receive,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Define,
    Plain,
    Compound { divides: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    Break,
    Continue,
    Goto,
    Fallthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Var,
    Const,
    Type,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Ident(IdentId),
    BasicLit(String),
    CompositeLit {
        typ: Option<Box<Self>>,
        elements: Vec<Self>,
    },
    KeyValue {
        key: Box<Self>,
        value: Box<Self>,
    },
    FuncLit {
        typ: FuncType,
        body: Vec<Stmt>,
    },
    Paren(Box<Self>),
    Selector {
        operand: Box<Self>,
        field: IdentId,
    },
    Index {
        operand: Box<Self>,
        indices: Vec<Self>,
    },
    Slice {
        operand: Box<Self>,
        bounds: Vec<Self>,
    },
    TypeAssert {
        operand: Box<Self>,
        typ: Option<Box<Self>>,
    },
    Call {
        function: Box<Self>,
        arguments: Vec<Self>,
    },
    Star(Box<Self>),
    Unary {
        op: UnaryOp,
        operand: Box<Self>,
    },
    Binary {
        may_panic: bool,
        left: Box<Self>,
        right: Box<Self>,
    },
    Ellipsis(Option<Box<Self>>),
    ArrayType {
        length: Option<Box<Self>>,
        element: Box<Self>,
    },
    StructType(Vec<Field>),
    FuncType(FuncType),
    InterfaceType(Vec<Field>),
    MapType {
        key: Box<Self>,
        value: Box<Self>,
    },
    ChanType(Box<Self>),
}

impl Expr {
    pub fn unparen(&self) -> &Self {
        if let Self::Paren(inner) = self {
            inner.unparen()
        } else {
            self
        }
    }

    pub fn unparen_ident(&self) -> Option<IdentId> {
        if let Self::Ident(ident) = self.unparen() {
            Some(*ident)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub names: Vec<IdentId>,
    pub typ: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FuncType {
    pub type_params: Option<Vec<Field>>,
    pub params: Vec<Field>,
    pub results: Option<Vec<Field>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Decl(GenDecl),
    Empty,
    Labeled {
        label: IdentId,
        statement: Box<Self>,
    },
    Expr(Expr),
    Send {
        channel: Expr,
        value: Expr,
    },
    IncDec(Expr),
    Assign {
        left: Vec<Expr>,
        op: AssignOp,
        right: Vec<Expr>,
    },
    Go(Expr),
    Defer(Expr),
    Return(Vec<Expr>),
    Branch {
        kind: BranchKind,
        label: Option<IdentId>,
    },
    Block(Vec<Self>),
    If(Box<IfStmt>),
    Case {
        list: Option<Vec<Expr>>,
        body: Vec<Self>,
    },
    Switch {
        init: Option<Box<Self>>,
        tag: Option<Expr>,
        body: Vec<Self>,
    },
    TypeSwitch {
        init: Option<Box<Self>>,
        assign: Box<Self>,
        body: Vec<Self>,
    },
    Comm {
        communication: Option<Box<Self>>,
        body: Vec<Self>,
    },
    Select(Vec<Self>),
    For(Box<ForStmt>),
    Range(Box<RangeStmt>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub init: Option<Stmt>,
    pub condition: Expr,
    pub body: Vec<Stmt>,
    pub alternative: Option<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStmt {
    pub init: Option<Stmt>,
    pub condition: Option<Expr>,
    pub post: Option<Stmt>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeStmt {
    pub key: Option<Expr>,
    pub value: Option<Expr>,
    pub define: bool,
    pub operand: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenDecl {
    pub kind: DeclKind,
    pub specs: Vec<Spec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spec {
    Value {
        names: Vec<IdentId>,
        typ: Option<Expr>,
        values: Vec<Expr>,
    },
    Type {
        name: IdentId,
        type_params: Option<Vec<Field>>,
        typ: Expr,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    pub receiver: Option<Vec<Field>>,
    pub name: IdentId,
    pub typ: FuncType,
    pub body: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    Gen(GenDecl),
    Func(FuncDecl),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct File {
    pub idents: Vec<Ident>,
    pub decls: Vec<Decl>,
}

impl File {
    pub fn ident(&self, id: IdentId) -> Option<&Ident> {
        self.idents.get(id)
    }

    pub fn name(&self, id: IdentId) -> &str {
        self.ident(id).map_or("", |ident| ident.name.as_str())
    }
}
