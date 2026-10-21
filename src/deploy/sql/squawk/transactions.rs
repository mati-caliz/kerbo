use super::statement::{Transaction, transaction, violation};
use super::types::create_table_arguments;
use crate::deploy::Violation;
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::{Token, TokenKind};
use crate::rules::MigrationRule;

const LOCK_TIMEOUT: &str = "lock_timeout";
const STATEMENT_TIMEOUT: &str = "statement_timeout";
const SLOW_FIRST_WORDS: &[&str] = &[
    "alter", "drop", "cluster", "comment", "import", "load", "lock", "refresh", "reindex", "truncate",
    "vacuum",
];
const TABLE_QUALIFIERS: &[&str] = &["global", "local", "temp", "temporary", "unlogged"];
const MAX_IDENTIFIER_BYTES: usize = 63;
const ALEMBIC_VERSION: &str = "alembic_version";

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    timeouts(statements, found);
    nesting(statements, found);
    concurrent_in_transaction(statements, found);
    long_identifiers(statements, found);
}

fn timeouts(statements: &[&[Token]], found: &mut Vec<Violation>) {
    let mut lock_missing = true;
    let mut statement_missing = true;
    for statement in statements {
        if let Some(name) = set_parameter(statement) {
            lock_missing = lock_missing && name != LOCK_TIMEOUT;
            statement_missing = statement_missing && name != STATEMENT_TIMEOUT;
            continue;
        }
        if !possibly_slow(statement) {
            continue;
        }
        let Some(first) = statement.first() else {
            continue;
        };
        if lock_missing {
            found.push(violation(MigrationRule::RequireLockTimeout, first, ""));
        }
        if statement_missing {
            found.push(violation(MigrationRule::RequireStatementTimeout, first, ""));
        }
        return;
    }
}

fn set_parameter(statement: &[Token]) -> Option<String> {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat("set") {
        return None;
    }
    cursor.eat_any(&["session", "local"]);
    let name = cursor.qualified_name();
    name.last()
        .and_then(|part| part.identifier())
        .or_else(|| Some(String::new()))
}

fn possibly_slow(statement: &[Token]) -> bool {
    let Some(first) = statement.first().and_then(Token::word) else {
        return false;
    };
    if SLOW_FIRST_WORDS.contains(&first) {
        return true;
    }
    if first == "select" {
        return statement.iter().any(|token| token.is_word("into"));
    }
    if first != "create" {
        return false;
    }
    let mut cursor = Cursor::new(statement);
    cursor.eat("create");
    cursor.eat_sequence(&["or", "replace"]);
    while cursor.eat_any(TABLE_QUALIFIERS) {}
    if !cursor.eat("table") {
        return true;
    }
    create_table_arguments(statement).map_or_else(
        || statement.iter().any(|token| token.is_word("as")),
        |arguments| arguments.iter().any(|token| token.is_word("references")),
    )
}

fn nesting(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        let boundary = !matches!(transaction(statement), Transaction::Other);
        if let Some(first) = statement.first().filter(|_| boundary) {
            found.push(violation(MigrationRule::TransactionNesting, first, ""));
        }
    }
}

fn concurrent_in_transaction(statements: &[&[Token]], found: &mut Vec<Violation>) {
    if statements.len() <= 1 {
        return;
    }
    let mut inside_transaction = true;
    for statement in statements {
        match transaction(statement) {
            Transaction::Begin => inside_transaction = true,
            Transaction::Commit { .. } => inside_transaction = false,
            Transaction::Rollback { .. } | Transaction::Other => {}
        }
        let mut cursor = Cursor::new(statement);
        if !cursor.eat("create") {
            continue;
        }
        cursor.eat("unique");
        if !cursor.eat("index") {
            continue;
        }
        if let Some(concurrently) = cursor.peek().filter(|token| token.is_word("concurrently"))
            && inside_transaction
        {
            found.push(violation(
                MigrationRule::BanConcurrentIndexCreationInTransaction,
                concurrently,
                "",
            ));
        }
    }
}

fn long_identifiers(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for token in statements.iter().flat_map(|statement| statement.iter()) {
        let name = match &token.kind {
            TokenKind::Word(name) | TokenKind::Quoted(name) => name,
            TokenKind::Literal | TokenKind::Symbol(_) => continue,
        };
        if name.len() > MAX_IDENTIFIER_BYTES {
            found.push(violation(MigrationRule::IdentifierTooLong, token, name));
        }
    }
}

pub fn alembic_version_transaction(statements: &[&[Token]]) -> bool {
    let mut start = None;
    let mut updates_version = false;
    for statement in statements {
        match transaction(statement) {
            Transaction::Begin => {
                start = Some(());
                updates_version = false;
            }
            Transaction::Commit { .. } | Transaction::Rollback { .. } => {
                if start.take().is_some() && updates_version {
                    return true;
                }
                updates_version = false;
            }
            Transaction::Other => {
                updates_version = updates_version || (start.is_some() && updates_alembic_version(statement));
            }
        }
    }
    false
}

fn updates_alembic_version(statement: &[Token]) -> bool {
    let mut cursor = Cursor::new(statement);
    cursor.eat("update") && {
        cursor.eat("only");
        cursor
            .qualified_name()
            .last()
            .and_then(|part| part.identifier())
            .is_some_and(|name| name.eq_ignore_ascii_case(ALEMBIC_VERSION))
    }
}
