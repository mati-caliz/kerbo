use super::statement::{alter_table, violation};
use crate::deploy::Violation;
use crate::deploy::sql::columns::{ColumnType, TypeKind, column, column_type, table_columns};
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const TABLE_QUALIFIERS: &[&str] = &["global", "local", "temp", "temporary", "unlogged"];
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const TABLE_CONSTRAINT_STARTS: &[&str] = &["constraint", "primary", "unique", "foreign", "check", "exclude"];
const TYPE_ENDS: &[&str] = &["using", "collate"];
const INT_TYPES: &[&str] = &["int", "integer", "int4", "serial", "serial4"];
const SMALLINT_TYPES: &[&str] = &["smallint", "int2", "smallserial", "serial2"];
const SERIAL_TYPES: &[&str] = &[
    "serial",
    "serial2",
    "serial4",
    "serial8",
    "smallserial",
    "bigserial",
];
const VARCHAR_NAME: &str = "varchar";

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        for typ in column_types(statement) {
            check(&typ, found);
        }
    }
}

fn check(typ: &ColumnType<'_>, found: &mut Vec<Violation>) {
    let rules = [
        (MigrationRule::PreferBigintOverInt, path_in(&typ.kind, INT_TYPES)),
        (
            MigrationRule::PreferBigintOverSmallint,
            path_in(&typ.kind, SMALLINT_TYPES),
        ),
        (MigrationRule::PreferIdentity, path_in(&typ.kind, SERIAL_TYPES)),
        (MigrationRule::PreferTextField, sized_varchar(&typ.kind)),
        (
            MigrationRule::PreferTimestampTz,
            timestamp_without_zone(&typ.kind),
        ),
    ];
    for (rule, applies) in rules {
        if applies {
            found.push(violation(rule, typ.start, ""));
        }
    }
}

fn path_in(kind: &TypeKind, names: &[&str]) -> bool {
    match kind {
        TypeKind::Array(inner) => path_in(inner, names),
        TypeKind::Path { name, .. } => names.contains(&name.as_str()),
        TypeKind::Varchar { .. }
        | TypeKind::Character
        | TypeKind::Bit
        | TypeKind::Double
        | TypeKind::Time
        | TypeKind::Timestamp { .. }
        | TypeKind::Interval => false,
    }
}

fn sized_varchar(kind: &TypeKind) -> bool {
    match kind {
        TypeKind::Array(inner) => sized_varchar(inner),
        TypeKind::Path { name, arguments } => name == VARCHAR_NAME && *arguments,
        TypeKind::Varchar { arguments } => *arguments,
        TypeKind::Character
        | TypeKind::Bit
        | TypeKind::Double
        | TypeKind::Time
        | TypeKind::Timestamp { .. }
        | TypeKind::Interval => false,
    }
}

fn timestamp_without_zone(kind: &TypeKind) -> bool {
    match kind {
        TypeKind::Array(inner) => timestamp_without_zone(inner),
        TypeKind::Path { name, arguments } => name == VARCHAR_NAME && *arguments,
        TypeKind::Timestamp { with_time_zone } => !with_time_zone,
        TypeKind::Varchar { .. }
        | TypeKind::Character
        | TypeKind::Bit
        | TypeKind::Double
        | TypeKind::Time
        | TypeKind::Interval => false,
    }
}

pub fn column_types(statement: &[Token]) -> Vec<ColumnType<'_>> {
    if let Some(arguments) = create_table_arguments(statement) {
        return table_columns(arguments)
            .into_iter()
            .filter_map(|column| column.typ)
            .collect();
    }
    let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) else {
        return Vec::new();
    };
    altered.actions.into_iter().filter_map(action_type).collect()
}

fn action_type(action: &[Token]) -> Option<ColumnType<'_>> {
    let mut cursor = Cursor::new(action);
    if cursor.eat("add") {
        if cursor
            .peek()
            .is_some_and(|token| TABLE_CONSTRAINT_STARTS.iter().any(|word| token.is_word(word)))
        {
            return None;
        }
        cursor.eat("column");
        cursor.eat_sequence(&["if", "not", "exists"]);
        cursor.next_token();
        return column(cursor.rest()).typ;
    }
    if !cursor.eat("alter") {
        return None;
    }
    cursor.eat("column");
    cursor.next_token();
    cursor.eat_sequence(&["set", "data"]);
    if !cursor.eat("type") {
        return None;
    }
    let rest = cursor.rest();
    let end = rest
        .iter()
        .position(|token| TYPE_ENDS.iter().any(|word| token.is_word(word)))
        .unwrap_or(rest.len());
    column_type(rest.get(..end).unwrap_or_default())
}

pub fn create_table_arguments(statement: &[Token]) -> Option<&[Token]> {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat("create") {
        return None;
    }
    cursor.eat_sequence(&["or", "replace"]);
    while cursor.eat_any(TABLE_QUALIFIERS) {}
    if !cursor.eat("table") {
        return None;
    }
    cursor.eat_sequence(&["if", "not", "exists"]);
    cursor.qualified_name();
    let rest = cursor.rest();
    if !rest
        .first()
        .is_some_and(|token| token.is_symbol(OPEN_PARENTHESIS))
    {
        return None;
    }
    let mut depth = 0_usize;
    for (index, token) in rest.iter().enumerate() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return rest.get(1..index);
            }
        }
    }
    rest.get(1..)
}
