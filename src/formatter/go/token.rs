#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Illegal,
    Eof,
    Ident,
    Int,
    Float,
    Imag,
    Char,
    String,
    Add,
    Sub,
    Mul,
    Quo,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    AndNot,
    AddAssign,
    SubAssign,
    MulAssign,
    QuoAssign,
    RemAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
    AndNotAssign,
    Land,
    Lor,
    Arrow,
    Inc,
    Dec,
    Eql,
    Lss,
    Gtr,
    Assign,
    Not,
    Neq,
    Leq,
    Geq,
    Define,
    Ellipsis,
    Lparen,
    Lbrack,
    Lbrace,
    Comma,
    Period,
    Rparen,
    Rbrack,
    Rbrace,
    Semicolon,
    Colon,
    Tilde,
    Break,
    Case,
    Chan,
    Const,
    Continue,
    Default,
    Defer,
    Else,
    Fallthrough,
    For,
    Func,
    Go,
    Goto,
    If,
    Import,
    Interface,
    Map,
    Package,
    Range,
    Return,
    Select,
    Struct,
    Switch,
    Type,
    Var,
}

pub const LOWEST_PRECEDENCE: i64 = 0;
pub const UNARY_PRECEDENCE: i64 = 6;
pub const HIGHEST_PRECEDENCE: i64 = 7;

const OR_PRECEDENCE: i64 = 1;
const AND_PRECEDENCE: i64 = 2;
const COMPARISON_PRECEDENCE: i64 = 3;
const ADDITION_PRECEDENCE: i64 = 4;
const MULTIPLICATION_PRECEDENCE: i64 = 5;

const TEXTS: [(Token, &str); 81] = [
    (Token::Illegal, "ILLEGAL"),
    (Token::Eof, "EOF"),
    (Token::Ident, "IDENT"),
    (Token::Int, "INT"),
    (Token::Float, "FLOAT"),
    (Token::Imag, "IMAG"),
    (Token::Char, "CHAR"),
    (Token::String, "STRING"),
    (Token::Add, "+"),
    (Token::Sub, "-"),
    (Token::Mul, "*"),
    (Token::Quo, "/"),
    (Token::Rem, "%"),
    (Token::And, "&"),
    (Token::Or, "|"),
    (Token::Xor, "^"),
    (Token::Shl, "<<"),
    (Token::Shr, ">>"),
    (Token::AndNot, "&^"),
    (Token::AddAssign, "+="),
    (Token::SubAssign, "-="),
    (Token::MulAssign, "*="),
    (Token::QuoAssign, "/="),
    (Token::RemAssign, "%="),
    (Token::AndAssign, "&="),
    (Token::OrAssign, "|="),
    (Token::XorAssign, "^="),
    (Token::ShlAssign, "<<="),
    (Token::ShrAssign, ">>="),
    (Token::AndNotAssign, "&^="),
    (Token::Land, "&&"),
    (Token::Lor, "||"),
    (Token::Arrow, "<-"),
    (Token::Inc, "++"),
    (Token::Dec, "--"),
    (Token::Eql, "=="),
    (Token::Lss, "<"),
    (Token::Gtr, ">"),
    (Token::Assign, "="),
    (Token::Not, "!"),
    (Token::Neq, "!="),
    (Token::Leq, "<="),
    (Token::Geq, ">="),
    (Token::Define, ":="),
    (Token::Ellipsis, "..."),
    (Token::Lparen, "("),
    (Token::Lbrack, "["),
    (Token::Lbrace, "{"),
    (Token::Comma, ","),
    (Token::Period, "."),
    (Token::Rparen, ")"),
    (Token::Rbrack, "]"),
    (Token::Rbrace, "}"),
    (Token::Semicolon, ";"),
    (Token::Colon, ":"),
    (Token::Tilde, "~"),
    (Token::Break, "break"),
    (Token::Case, "case"),
    (Token::Chan, "chan"),
    (Token::Const, "const"),
    (Token::Continue, "continue"),
    (Token::Default, "default"),
    (Token::Defer, "defer"),
    (Token::Else, "else"),
    (Token::Fallthrough, "fallthrough"),
    (Token::For, "for"),
    (Token::Func, "func"),
    (Token::Go, "go"),
    (Token::Goto, "goto"),
    (Token::If, "if"),
    (Token::Import, "import"),
    (Token::Interface, "interface"),
    (Token::Map, "map"),
    (Token::Package, "package"),
    (Token::Range, "range"),
    (Token::Return, "return"),
    (Token::Select, "select"),
    (Token::Struct, "struct"),
    (Token::Switch, "switch"),
    (Token::Type, "type"),
    (Token::Var, "var"),
];

impl Token {
    pub fn from_text(text: &str) -> Option<Self> {
        TEXTS
            .iter()
            .find(|(_, candidate)| *candidate == text)
            .map(|(token, _)| *token)
    }

    pub const fn precedence(self) -> i64 {
        if matches!(self, Self::Lor) {
            OR_PRECEDENCE
        } else if matches!(self, Self::Land) {
            AND_PRECEDENCE
        } else if matches!(
            self,
            Self::Eql | Self::Neq | Self::Lss | Self::Leq | Self::Gtr | Self::Geq
        ) {
            COMPARISON_PRECEDENCE
        } else if matches!(self, Self::Add | Self::Sub | Self::Or | Self::Xor) {
            ADDITION_PRECEDENCE
        } else if matches!(
            self,
            Self::Mul | Self::Quo | Self::Rem | Self::Shl | Self::Shr | Self::And | Self::AndNot
        ) {
            MULTIPLICATION_PRECEDENCE
        } else {
            LOWEST_PRECEDENCE
        }
    }

    pub fn text(self) -> &'static str {
        TEXTS
            .iter()
            .find(|(token, _)| *token == self)
            .map_or("", |(_, text)| text)
    }

    pub const fn implies_semicolon(self) -> bool {
        matches!(
            self,
            Self::Break
                | Self::Continue
                | Self::Fallthrough
                | Self::Return
                | Self::Inc
                | Self::Dec
                | Self::Rparen
                | Self::Rbrack
                | Self::Rbrace
        )
    }

    // Igual que `mayCombine` de go/printer: estos pares, pegados, se leerían como otro token.
    pub const fn combines_with(self, next: u8) -> bool {
        matches!(
            (self, next),
            (Self::Int, b'.')
                | (Self::Add, b'+')
                | (Self::Sub, b'-')
                | (Self::Quo, b'*')
                | (Self::Lss, b'-' | b'<')
                | (Self::And, b'&' | b'^')
        )
    }
}
