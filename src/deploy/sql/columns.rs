use super::cursor::{Cursor, top_level_items};
use super::lexer::{Token, TokenKind};

const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const OPEN_BRACKET: char = '[';
const QUALIFIER_SEPARATOR: char = '.';
const ARRAY_WORD: &str = "array";
const CONSTRAINT_STARTS: &[&str] = &[
    "constraint",
    "not",
    "null",
    "default",
    "primary",
    "unique",
    "references",
    "check",
    "generated",
    "collate",
    "compression",
    "storage",
    "deferrable",
    "initially",
    "using",
];
const CONSTRAINT_KEYWORDS: &[&str] = &[
    "constraint",
    "not",
    "null",
    "default",
    "primary",
    "unique",
    "references",
    "check",
    "generated",
];
const CLAUSE_CONTINUATIONS: &[&str] = &["set", "by", "default"];
const TABLE_CONSTRAINT_STARTS: &[&str] = &[
    "constraint",
    "primary",
    "unique",
    "foreign",
    "check",
    "exclude",
    "like",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Path { name: String, arguments: bool },
    Varchar { arguments: bool },
    Character,
    Bit,
    Double,
    Time,
    Timestamp { with_time_zone: bool },
    Interval,
    Array(Box<Self>),
}

#[derive(Debug, Clone)]
pub struct ColumnType<'tokens> {
    pub start: &'tokens Token,
    pub kind: TypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    References,
    ForeignKey,
    Unique,
    PrimaryKey,
    Default,
    Generated,
    Check,
    Other,
}

#[derive(Debug, Clone)]
pub struct Constraint<'tokens> {
    pub start: &'tokens Token,
    pub kind: ConstraintKind,
    pub body: &'tokens [Token],
    pub name: Option<String>,
}

impl Constraint<'_> {
    pub fn is_not_valid(&self) -> bool {
        self.body
            .windows(2)
            .any(|pair| matches!(pair, [not, valid] if not.is_word("not") && valid.is_word("valid")))
    }

    pub fn uses_index(&self) -> bool {
        self.body
            .windows(2)
            .any(|pair| matches!(pair, [using, index] if using.is_word("using") && index.is_word("index")))
    }
}

#[derive(Debug, Clone)]
pub struct Column<'tokens> {
    pub typ: Option<ColumnType<'tokens>>,
    pub constraints: Vec<Constraint<'tokens>>,
}

pub fn table_columns(arguments: &[Token]) -> Vec<Column<'_>> {
    top_level_items(arguments)
        .into_iter()
        .filter(|item| {
            item.first()
                .is_some_and(|first| !TABLE_CONSTRAINT_STARTS.iter().any(|word| first.is_word(word)))
        })
        .map(|item| column(item.get(1..).unwrap_or_default()))
        .collect()
}

pub fn column(after_name: &[Token]) -> Column<'_> {
    let type_length = after_name
        .iter()
        .position(|token| CONSTRAINT_STARTS.iter().any(|word| token.is_word(word)))
        .unwrap_or(after_name.len());
    let (type_tokens, rest) = after_name.split_at(type_length);
    Column {
        typ: column_type(type_tokens),
        constraints: column_constraints(rest),
    }
}

pub fn column_type(tokens: &[Token]) -> Option<ColumnType<'_>> {
    let start = tokens.first()?;
    Some(ColumnType {
        start,
        kind: type_kind(tokens),
    })
}

fn type_kind(tokens: &[Token]) -> TypeKind {
    let array_start = tokens.iter().enumerate().position(|(index, token)| {
        index > 0
            && (token.is_symbol(OPEN_BRACKET) || token.is_word(ARRAY_WORD))
            && !inside_arguments(tokens, index)
    });
    if let Some(array_start) = array_start {
        return TypeKind::Array(Box::new(type_kind(tokens.get(..array_start).unwrap_or_default())));
    }
    let words: Vec<&str> = tokens.iter().filter_map(Token::word).collect();
    let arguments = tokens.iter().any(|token| token.is_symbol(OPEN_PARENTHESIS));
    let first = tokens.first().and_then(Token::word).unwrap_or_default();
    match first {
        "timestamp" => TypeKind::Timestamp {
            with_time_zone: words.contains(&"with"),
        },
        "time" => TypeKind::Time,
        "interval" => TypeKind::Interval,
        "double" => TypeKind::Double,
        "bit" => TypeKind::Bit,
        "varchar" => TypeKind::Varchar { arguments },
        "character" | "char" | "nchar" | "national" if words.contains(&"varying") => {
            TypeKind::Varchar { arguments }
        }
        "character" | "char" | "nchar" | "national" => TypeKind::Character,
        _ => TypeKind::Path {
            name: path_name(tokens),
            arguments,
        },
    }
}

fn inside_arguments(tokens: &[Token], index: usize) -> bool {
    let mut depth = 0_usize;
    for token in tokens.get(..index).unwrap_or_default() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
        }
    }
    depth > 0
}

fn path_name(tokens: &[Token]) -> String {
    let mut name = String::new();
    let mut previous_was_separator = true;
    for token in tokens {
        if token.is_symbol(QUALIFIER_SEPARATOR) {
            previous_was_separator = true;
            continue;
        }
        if !previous_was_separator {
            break;
        }
        match &token.kind {
            TokenKind::Word(word) | TokenKind::Quoted(word) => name.clone_from(word),
            TokenKind::Literal | TokenKind::Symbol(_) => break,
        }
        previous_was_separator = false;
    }
    name
}

pub fn column_constraints(tokens: &[Token]) -> Vec<Constraint<'_>> {
    let starts: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| starts_constraint(tokens, *index, token))
        .map(|(index, _)| index)
        .collect();
    starts
        .iter()
        .enumerate()
        .filter_map(|(position, start)| {
            let end = starts
                .get(position.saturating_add(1))
                .copied()
                .unwrap_or(tokens.len());
            tokens.get(*start..end).and_then(constraint)
        })
        .collect()
}

fn starts_constraint(tokens: &[Token], index: usize, token: &Token) -> bool {
    if inside_arguments(tokens, index) {
        return false;
    }
    let after_constraint_name = index
        .checked_sub(2)
        .and_then(|before| tokens.get(before))
        .is_some_and(|before| before.is_word("constraint"));
    let previous = index.checked_sub(1).and_then(|before| tokens.get(before));
    let continues_clause = previous.is_some_and(|before| {
        CLAUSE_CONTINUATIONS.iter().any(|word| before.is_word(word))
            || (token.is_word("null") && before.is_word("not"))
    });
    let keyword = CONSTRAINT_KEYWORDS.iter().any(|word| token.is_word(word));
    keyword && !after_constraint_name && !continues_clause
}

pub fn constraint(tokens: &[Token]) -> Option<Constraint<'_>> {
    let start = tokens.first()?;
    let mut cursor = Cursor::new(tokens);
    let name = if cursor.eat("constraint") {
        cursor.next_token().and_then(Token::identifier)
    } else {
        None
    };
    let kind = if cursor.eat("references") {
        ConstraintKind::References
    } else if cursor.eat_sequence(&["foreign", "key"]) {
        ConstraintKind::ForeignKey
    } else if cursor.eat("unique") {
        ConstraintKind::Unique
    } else if cursor.eat_sequence(&["primary", "key"]) {
        ConstraintKind::PrimaryKey
    } else if cursor.eat("default") {
        ConstraintKind::Default
    } else if cursor.eat("generated") {
        ConstraintKind::Generated
    } else if cursor.eat("check") {
        ConstraintKind::Check
    } else {
        ConstraintKind::Other
    };
    Some(Constraint {
        start,
        kind,
        body: cursor.rest(),
        name,
    })
}
