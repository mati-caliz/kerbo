#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Add,
    Sub,
    Mult,
    MatMult,
    Div,
    Mod,
    Pow,
    LShift,
    RShift,
    BitOr,
    BitXor,
    BitAnd,
    FloorDiv,
}

impl Operator {
    pub const fn from_token(token: &str) -> Option<Self> {
        Some(match token.as_bytes() {
            b"+" | b"+=" => Self::Add,
            b"-" | b"-=" => Self::Sub,
            b"*" | b"*=" => Self::Mult,
            b"@" | b"@=" => Self::MatMult,
            b"/" | b"/=" => Self::Div,
            b"%" | b"%=" => Self::Mod,
            b"**" | b"**=" => Self::Pow,
            b"<<" | b"<<=" => Self::LShift,
            b">>" | b">>=" => Self::RShift,
            b"|" | b"|=" => Self::BitOr,
            b"^" | b"^=" => Self::BitXor,
            b"&" | b"&=" => Self::BitAnd,
            b"//" | b"//=" => Self::FloorDiv,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mult => "*",
            Self::MatMult => "@",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Pow => "**",
            Self::LShift => "<<",
            Self::RShift => ">>",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::BitAnd => "&",
            Self::FloorDiv => "//",
        }
    }

    pub const fn as_augmented_str(self) -> &'static str {
        match self {
            Self::Add => "+=",
            Self::Sub => "-=",
            Self::Mult => "*=",
            Self::MatMult => "@=",
            Self::Div => "/=",
            Self::Mod => "%=",
            Self::Pow => "**=",
            Self::LShift => "<<=",
            Self::RShift => ">>=",
            Self::BitOr => "|=",
            Self::BitXor => "^=",
            Self::BitAnd => "&=",
            Self::FloorDiv => "//=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOperator {
    And,
    Or,
}

impl BoolOperator {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::And => "and",
            Self::Or => "or",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Invert,
    Not,
    UAdd,
    USub,
}

impl UnaryOperator {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invert => "~",
            Self::Not => "not",
            Self::UAdd => "+",
            Self::USub => "-",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOperator {
    Eq,
    NotEq,
    Lt,
    LtE,
    Gt,
    GtE,
    Is,
    IsNot,
    In,
    NotIn,
}

impl CmpOperator {
    pub const fn from_token(token: &str) -> Option<Self> {
        Some(match token.as_bytes() {
            b"==" => Self::Eq,
            b"!=" => Self::NotEq,
            b"<" => Self::Lt,
            b"<=" => Self::LtE,
            b">" => Self::Gt,
            b">=" => Self::GtE,
            b"is" => Self::Is,
            b"is not" => Self::IsNot,
            b"in" => Self::In,
            b"not in" => Self::NotIn,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::NotEq => "!=",
            Self::Lt => "<",
            Self::LtE => "<=",
            Self::Gt => ">",
            Self::GtE => ">=",
            Self::Is => "is",
            Self::IsNot => "is not",
            Self::In => "in",
            Self::NotIn => "not in",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberKind {
    Int,
    Float,
    Complex,
}
