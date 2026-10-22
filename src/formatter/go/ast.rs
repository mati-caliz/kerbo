mod declarations;
mod statements;

pub use declarations::{Decl, File, FuncDecl, GenDecl, ImportSpec, Spec, TypeSpec, ValueSpec};
pub use statements::{
    AssignStmt, Block, BranchStmt, CallStmt, CaseClause, CommClause, ForStmt, IfStmt, IncDecStmt,
    LabeledStmt, RangeStmt, ReturnStmt, SelectStmt, SendStmt, Stmt, SwitchStmt, TypeSwitchStmt,
};

use super::position::Pos;
use super::token::Token;

const ELLIPSIS_LENGTH: usize = 3;

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub pos: Pos,
}

impl Ident {
    pub const fn end(&self) -> Pos {
        self.pos.shifted(self.name.len())
    }
}

#[derive(Debug, Clone)]
pub struct BasicLit {
    pub kind: Token,
    pub value: String,
    pub pos: Pos,
    pub end: Pos,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub slash: Pos,
    pub text: String,
}

impl Comment {
    pub const fn end(&self) -> Pos {
        self.slash.shifted(self.text.len())
    }

    pub fn is_line_comment(&self) -> bool {
        self.text.as_bytes().get(1) == Some(&b'/')
    }
}

#[derive(Debug, Clone)]
pub struct CommentGroup {
    pub list: Vec<Comment>,
}

impl CommentGroup {
    pub fn pos(&self) -> Pos {
        self.list.first().map_or(Pos::NONE, |comment| comment.slash)
    }

    pub fn end(&self) -> Pos {
        self.list.last().map_or(Pos::NONE, Comment::end)
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub names: Vec<Ident>,
    pub typ: Expr,
    pub tag: Option<BasicLit>,
    pub comment: Option<usize>,
}

impl Field {
    pub fn pos(&self) -> Pos {
        self.names.first().map_or_else(|| self.typ.pos(), |name| name.pos)
    }

    pub fn end(&self) -> Pos {
        self.tag.as_ref().map_or_else(|| self.typ.end(), |tag| tag.end)
    }
}

#[derive(Debug, Clone)]
pub struct FieldList {
    pub opening: Pos,
    pub list: Vec<Field>,
    pub closing: Pos,
}

impl FieldList {
    pub fn pos(&self) -> Pos {
        if self.opening.is_valid() {
            return self.opening;
        }
        self.list.first().map_or(Pos::NONE, Field::pos)
    }

    pub fn end(&self) -> Pos {
        if self.closing.is_valid() {
            return self.closing.shifted(1);
        }
        self.list.last().map_or(Pos::NONE, Field::end)
    }

    pub fn field_count(&self) -> usize {
        self.list.iter().map(|field| field.names.len().max(1)).sum()
    }
}

#[derive(Debug, Clone)]
pub struct FuncType {
    pub func: Pos,
    pub type_params: Option<FieldList>,
    pub params: FieldList,
    pub results: Option<FieldList>,
}

impl FuncType {
    pub fn pos(&self) -> Pos {
        if self.func.is_valid() {
            self.func
        } else {
            self.params.pos()
        }
    }

    pub fn end(&self) -> Pos {
        self.results
            .as_ref()
            .map_or_else(|| self.params.end(), FieldList::end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChanDir {
    Both,
    Send,
    Receive,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(Ident),
    BasicLit(BasicLit),
    Composite(Box<CompositeLit>),
    FuncLit(Box<FuncLit>),
    Paren(Box<Paren>),
    Selector(Box<Selector>),
    Index(Box<Index>),
    IndexList(Box<IndexList>),
    Slice(Box<Slice>),
    TypeAssert(Box<TypeAssert>),
    Call(Box<Call>),
    Star(Box<Star>),
    Unary(Box<Unary>),
    Binary(Box<Binary>),
    KeyValue(Box<KeyValue>),
    Array(Box<ArrayType>),
    Struct(Box<StructType>),
    Func(Box<FuncType>),
    Interface(Box<InterfaceType>),
    Map(Box<MapType>),
    Chan(Box<ChanType>),
    Ellipsis(Box<Ellipsis>),
}

#[derive(Debug, Clone)]
pub struct CompositeLit {
    pub typ: Option<Expr>,
    pub lbrace: Pos,
    pub elts: Vec<Expr>,
    pub rbrace: Pos,
}

#[derive(Debug, Clone)]
pub struct FuncLit {
    pub typ: FuncType,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct Paren {
    pub lparen: Pos,
    pub x: Expr,
    pub rparen: Pos,
}

#[derive(Debug, Clone)]
pub struct Selector {
    pub x: Expr,
    pub sel: Ident,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub x: Expr,
    pub lbrack: Pos,
    pub subscript: Expr,
    pub rbrack: Pos,
}

#[derive(Debug, Clone)]
pub struct IndexList {
    pub x: Expr,
    pub lbrack: Pos,
    pub indices: Vec<Expr>,
    pub rbrack: Pos,
}

#[derive(Debug, Clone)]
pub struct Slice {
    pub x: Expr,
    pub lbrack: Pos,
    pub low: Option<Expr>,
    pub high: Option<Expr>,
    pub max: Option<Expr>,
    pub rbrack: Pos,
}

#[derive(Debug, Clone)]
pub struct TypeAssert {
    pub x: Expr,
    pub lparen: Pos,
    pub typ: Option<Expr>,
    pub rparen: Pos,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub fun: Expr,
    pub lparen: Pos,
    pub args: Vec<Expr>,
    pub ellipsis: Pos,
    pub rparen: Pos,
}

#[derive(Debug, Clone)]
pub struct Star {
    pub star: Pos,
    pub x: Expr,
}

#[derive(Debug, Clone)]
pub struct Unary {
    pub op_pos: Pos,
    pub op: Token,
    pub x: Expr,
}

#[derive(Debug, Clone)]
pub struct Binary {
    pub x: Expr,
    pub op_pos: Pos,
    pub op: Token,
    pub y: Expr,
}

#[derive(Debug, Clone)]
pub struct KeyValue {
    pub key: Expr,
    pub colon: Pos,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct ArrayType {
    pub lbrack: Pos,
    pub len: Option<Expr>,
    pub elt: Expr,
}

#[derive(Debug, Clone)]
pub struct StructType {
    pub keyword: Pos,
    pub fields: FieldList,
}

#[derive(Debug, Clone)]
pub struct InterfaceType {
    pub keyword: Pos,
    pub methods: FieldList,
}

#[derive(Debug, Clone)]
pub struct MapType {
    pub keyword: Pos,
    pub key: Expr,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct ChanType {
    pub begin: Pos,
    pub arrow: Pos,
    pub dir: ChanDir,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct Ellipsis {
    pub pos: Pos,
    pub elt: Option<Expr>,
}

impl Expr {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Ident(ident) => ident.pos,
            Self::BasicLit(literal) => literal.pos,
            Self::Composite(literal) => literal.typ.as_ref().map_or(literal.lbrace, Self::pos),
            Self::FuncLit(literal) => literal.typ.pos(),
            Self::Paren(paren) => paren.lparen,
            Self::Selector(selector) => selector.x.pos(),
            Self::Index(index) => index.x.pos(),
            Self::IndexList(index) => index.x.pos(),
            Self::Slice(slice) => slice.x.pos(),
            Self::TypeAssert(assert) => assert.x.pos(),
            Self::Call(call) => call.fun.pos(),
            Self::Star(star) => star.star,
            Self::Unary(unary) => unary.op_pos,
            Self::Binary(binary) => binary.x.pos(),
            Self::KeyValue(pair) => pair.key.pos(),
            Self::Array(array) => array.lbrack,
            Self::Struct(structure) => structure.keyword,
            Self::Func(func) => func.pos(),
            Self::Interface(interface) => interface.keyword,
            Self::Map(map) => map.keyword,
            Self::Chan(chan) => chan.begin,
            Self::Ellipsis(ellipsis) => ellipsis.pos,
        }
    }

    pub fn end(&self) -> Pos {
        match self {
            Self::Ident(ident) => ident.end(),
            Self::BasicLit(literal) => literal.end,
            Self::Composite(literal) => literal.rbrace.shifted(1),
            Self::FuncLit(literal) => literal.body.end(),
            Self::Paren(paren) => paren.rparen.shifted(1),
            Self::Selector(selector) => selector.sel.end(),
            Self::Index(index) => index.rbrack.shifted(1),
            Self::IndexList(index) => index.rbrack.shifted(1),
            Self::Slice(slice) => slice.rbrack.shifted(1),
            Self::TypeAssert(assert) => assert.rparen.shifted(1),
            Self::Call(call) => call.rparen.shifted(1),
            Self::Star(star) => star.x.end(),
            Self::Unary(unary) => unary.x.end(),
            Self::Binary(binary) => binary.y.end(),
            Self::KeyValue(pair) => pair.value.end(),
            Self::Array(array) => array.elt.end(),
            Self::Struct(structure) => structure.fields.end(),
            Self::Func(func) => func.end(),
            Self::Interface(interface) => interface.methods.end(),
            Self::Map(map) => map.value.end(),
            Self::Chan(chan) => chan.value.end(),
            Self::Ellipsis(ellipsis) => ellipsis
                .elt
                .as_ref()
                .map_or_else(|| ellipsis.pos.shifted(ELLIPSIS_LENGTH), Self::end),
        }
    }

    pub fn without_parens(&self) -> &Self {
        if let Self::Paren(paren) = self {
            paren.x.without_parens()
        } else {
            self
        }
    }
}
