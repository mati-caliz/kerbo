use super::cursor::{Cursor, last_raw, normalized, top_level_items, top_level_words};
use super::lexer::Token;
use super::{Migration, TableColumn};
use crate::rules::MigrationRule;

const TABLE_CONSTRAINTS: &[&str] = &["constraint", "primary", "unique", "foreign", "check", "exclude"];
const NOT_NULL_CHECK: [&str; 3] = ["is", "not", "null"];
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Table,
    ForeignTable,
}

struct Altered {
    target: Target,
    table: String,
    name: String,
}

pub(super) fn alter(migration: &mut Migration, mut cursor: Cursor<'_>) {
    if cursor.eat("table") {
        alter_table(migration, cursor, Target::Table);
    } else if cursor.eat_sequence(&["foreign", "table"]) {
        alter_table(migration, cursor, Target::ForeignTable);
    } else if cursor.eat("type") {
        alter_type(migration, cursor);
    } else if cursor.eat("view") || cursor.eat_sequence(&["materialized", "view"]) {
        alter_view(migration, cursor);
    }
}

fn alter_table(migration: &mut Migration, mut cursor: Cursor<'_>, target: Target) {
    cursor.eat_sequence(&["if", "exists"]);
    cursor.eat("only");
    let name = cursor.qualified_name();
    let Some(table) = last_raw(&name) else {
        return;
    };
    let altered = Altered {
        target,
        table,
        name: normalized(&name).join("."),
    };
    let rest = cursor.rest();
    let rest = rest
        .split_first()
        .filter(|(first, _)| first.is_symbol('*'))
        .map_or(rest, |(_, after)| after);
    for action in top_level_items(rest) {
        table_action(migration, &altered, action);
    }
}

fn table_action(migration: &mut Migration, altered: &Altered, action: &[Token]) {
    let Some(first) = action.first() else {
        return;
    };
    let mut cursor = Cursor::new(action);
    if cursor.eat("add") {
        add(migration, altered, first, cursor);
    } else if cursor.eat("alter") {
        alter_column(migration, altered, cursor);
    } else if cursor.eat("rename") {
        rename(migration, altered, first, cursor);
    } else if cursor.eat("drop") {
        drop_column(migration, first, cursor);
    } else if cursor.eat_sequence(&["validate", "constraint"]) {
        validate(migration, altered, cursor);
    }
}

fn add(migration: &mut Migration, altered: &Altered, first: &Token, mut cursor: Cursor<'_>) {
    if cursor
        .peek()
        .is_some_and(|token| TABLE_CONSTRAINTS.iter().any(|word| token.is_word(word)))
    {
        remember_not_null_check(migration, altered, cursor);
        return;
    }
    if altered.target != Target::Table {
        return;
    }
    cursor.eat("column");
    cursor.eat_sequence(&["if", "not", "exists"]);
    let column = cursor
        .next_token()
        .and_then(Token::identifier)
        .unwrap_or_default();
    let words = top_level_words(cursor.rest());
    let not_null = words.windows(2).any(|pair| pair == ["not", "null"]);
    let defaulted = words.contains(&"default") || words.contains(&"generated");
    if not_null && !defaulted {
        migration.report(MigrationRule::AddingRequiredField, first, column);
    }
}

fn remember_not_null_check(migration: &mut Migration, altered: &Altered, mut cursor: Cursor<'_>) {
    if !cursor.eat("constraint") {
        return;
    }
    let Some(name) = cursor.next_token().map(|token| token.raw.clone()) else {
        return;
    };
    if !cursor.eat("check") {
        return;
    }
    let rest = cursor.rest();
    let not_valid = top_level_words(rest)
        .windows(2)
        .any(|pair| pair == ["not", "valid"]);
    let Some(column) = checked_not_null_column(rest).filter(|_| not_valid) else {
        return;
    };
    migration.not_null_checks.insert(
        name,
        TableColumn {
            table: altered.table.clone(),
            column,
        },
    );
}

fn checked_not_null_column(tokens: &[Token]) -> Option<String> {
    let (open, rest) = tokens.split_first()?;
    let (column, rest) = rest.split_first()?;
    let (check, rest) = rest.split_at_checked(NOT_NULL_CHECK.len())?;
    let close = rest.first()?;
    let is_check = open.is_symbol(OPEN_PARENTHESIS)
        && close.is_symbol(CLOSE_PARENTHESIS)
        && column.identifier().is_some()
        && check
            .iter()
            .zip(NOT_NULL_CHECK)
            .all(|(token, word)| token.is_word(word));
    is_check.then(|| column.raw.clone())
}

fn validate(migration: &mut Migration, altered: &Altered, mut cursor: Cursor<'_>) {
    let Some(name) = cursor.next_token().map(|token| token.raw.clone()) else {
        return;
    };
    match migration.not_null_checks.get(&name) {
        Some(checked) if checked.table == altered.table => {
            let checked = checked.clone();
            migration.validated_columns.insert(checked);
        }
        Some(_) | None => {
            migration
                .externally_validated_tables
                .insert(altered.table.clone());
        }
    }
}

fn alter_column(migration: &mut Migration, altered: &Altered, mut cursor: Cursor<'_>) {
    cursor.eat("column");
    let Some(column) = cursor.next_token() else {
        return;
    };
    let Some(option) = cursor.peek() else {
        return;
    };
    let column_name = column.identifier().unwrap_or_default();
    if cursor.eat("type") || cursor.eat_sequence(&["set", "data", "type"]) {
        migration.report(MigrationRule::ChangingColumnType, option, column_name);
    } else if altered.target == Target::Table && cursor.eat_sequence(&["set", "not", "null"]) {
        let checked = TableColumn {
            table: altered.table.clone(),
            column: column.raw.clone(),
        };
        let safe = migration.validated_columns.contains(&checked)
            || migration.externally_validated_tables.contains(&altered.table);
        if !safe {
            migration.report(MigrationRule::AddingNotNullableField, option, column_name);
        }
    }
}

fn rename(migration: &mut Migration, altered: &Altered, first: &Token, mut cursor: Cursor<'_>) {
    if cursor.eat("constraint") {
        return;
    }
    if cursor.eat("to") {
        if altered.target == Target::Table {
            migration.report(MigrationRule::RenamingTable, first, altered.name.clone());
        }
        return;
    }
    cursor.eat("column");
    let column = cursor
        .next_token()
        .and_then(Token::identifier)
        .unwrap_or_default();
    migration.report(MigrationRule::RenamingColumn, first, column);
}

fn drop_column(migration: &mut Migration, first: &Token, mut cursor: Cursor<'_>) {
    if cursor.peek().is_some_and(|token| token.is_word("constraint")) {
        return;
    }
    cursor.eat("column");
    cursor.eat_sequence(&["if", "exists"]);
    let column = cursor
        .next_token()
        .and_then(Token::identifier)
        .unwrap_or_default();
    migration.report(MigrationRule::BanDropColumn, first, column);
}

fn alter_type(migration: &mut Migration, mut cursor: Cursor<'_>) {
    cursor.qualified_name();
    if let Some(first) = cursor.peek().filter(|token| token.is_word("rename")) {
        cursor.next_token();
        if cursor.eat("attribute") {
            let attribute = cursor
                .next_token()
                .and_then(Token::identifier)
                .unwrap_or_default();
            migration.report(MigrationRule::RenamingColumn, first, attribute);
        }
        return;
    }
    for action in top_level_items(cursor.rest()) {
        type_action(migration, action);
    }
}

fn type_action(migration: &mut Migration, action: &[Token]) {
    let Some(first) = action.first() else {
        return;
    };
    let mut cursor = Cursor::new(action);
    let rule = if cursor.eat_sequence(&["drop", "attribute"]) {
        cursor.eat_sequence(&["if", "exists"]);
        MigrationRule::BanDropColumn
    } else if cursor.eat_sequence(&["alter", "attribute"]) {
        MigrationRule::ChangingColumnType
    } else {
        return;
    };
    let attribute = cursor
        .next_token()
        .and_then(Token::identifier)
        .unwrap_or_default();
    migration.report(rule, first, attribute);
}

fn alter_view(migration: &mut Migration, mut cursor: Cursor<'_>) {
    cursor.eat_sequence(&["if", "exists"]);
    cursor.qualified_name();
    let Some(first) = cursor.peek().filter(|token| token.is_word("rename")) else {
        return;
    };
    cursor.next_token();
    if cursor.eat("to") {
        return;
    }
    cursor.eat("column");
    let column = cursor
        .next_token()
        .and_then(Token::identifier)
        .unwrap_or_default();
    migration.report(MigrationRule::RenamingColumn, first, column);
}
