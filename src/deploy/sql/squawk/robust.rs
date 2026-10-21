use std::collections::HashMap;

use super::statement::{Transaction, alter_table, transaction, violation};
use super::transactions::alembic_version_transaction;
use crate::deploy::Violation;
use crate::deploy::sql::columns::constraint;
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const TABLE_QUALIFIERS: &[&str] = &["global", "local", "temp", "temporary", "unlogged"];
const TEMPORARY: &[&str] = &["temp", "temporary"];
const TABLE_CONSTRAINT_STARTS: &[&str] = &["constraint", "primary", "unique", "foreign", "check", "exclude"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    Dropped,
    Added,
}

#[derive(Debug, Default)]
struct Robustness {
    names: HashMap<String, Named>,
}

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    let versioned = alembic_version_transaction(statements);
    let mut inside_transaction = true;
    let mut robustness = Robustness::default();
    for statement in statements {
        let robust = versioned || inside_transaction;
        match transaction(statement) {
            Transaction::Begin => inside_transaction = true,
            Transaction::Commit { .. } => inside_transaction = false,
            Transaction::Rollback { .. } | Transaction::Other => {}
        }
        if let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) {
            let unguarded: Vec<&Token> = altered
                .actions
                .iter()
                .filter(|action| robustness.needs_guard(action))
                .filter_map(|action| action.first())
                .collect();
            if !robust {
                found.extend(
                    unguarded
                        .into_iter()
                        .map(|first| violation(MigrationRule::PreferRobustStmts, first, "")),
                );
            }
            continue;
        }
        if unguarded_statement(statement, robust) {
            found.extend(
                statement
                    .first()
                    .map(|first| violation(MigrationRule::PreferRobustStmts, first, "")),
            );
        }
    }
}

impl Robustness {
    fn needs_guard(&mut self, action: &[Token]) -> bool {
        let mut cursor = Cursor::new(action);
        if cursor.eat_sequence(&["drop", "constraint"]) {
            let guarded = cursor.eat_sequence(&["if", "exists"]);
            if let Some(name) = cursor.next_token().and_then(Token::identifier) {
                self.names.insert(name, Named::Dropped);
            }
            return !guarded;
        }
        if cursor.eat_sequence(&["validate", "constraint"]) {
            return !cursor
                .next_token()
                .and_then(Token::identifier)
                .is_some_and(|name| self.names.contains_key(&name));
        }
        if cursor.eat("add") {
            return self.added_needs_guard(cursor);
        }
        if cursor.eat("drop") {
            cursor.eat("column");
            return !cursor.eat_sequence(&["if", "exists"]);
        }
        true
    }

    fn added_needs_guard(&mut self, mut cursor: Cursor<'_>) -> bool {
        let is_constraint = cursor
            .peek()
            .is_some_and(|token| TABLE_CONSTRAINT_STARTS.iter().any(|word| token.is_word(word)));
        if !is_constraint {
            cursor.eat("column");
            return !cursor.eat_sequence(&["if", "not", "exists"]);
        }
        let name = constraint(cursor.rest()).and_then(|added| added.name);
        if let Some(named) = name.and_then(|name| self.names.get_mut(&name))
            && *named == Named::Dropped
        {
            *named = Named::Added;
            return false;
        }
        true
    }
}

fn unguarded_statement(statement: &[Token], robust: bool) -> bool {
    let mut cursor = Cursor::new(statement);
    if cursor.eat("create") {
        return unguarded_creation(cursor, robust);
    }
    if !cursor.eat("drop") || robust {
        return false;
    }
    if cursor.eat_any(&["index", "table", "type"]) {
        cursor.eat("concurrently");
        return !cursor.eat_sequence(&["if", "exists"]);
    }
    false
}

fn unguarded_creation(mut cursor: Cursor<'_>, robust: bool) -> bool {
    cursor.eat("unique");
    if cursor.eat("index") {
        let concurrently = cursor.eat("concurrently");
        if cursor.eat_sequence(&["if", "not", "exists"]) {
            return false;
        }
        let named = cursor.peek().is_some_and(|token| !token.is_word("on"));
        return named && (concurrently || !robust);
    }
    cursor.eat_sequence(&["or", "replace"]);
    let mut temporary = false;
    while let Some(token) = cursor
        .peek()
        .filter(|token| TABLE_QUALIFIERS.iter().any(|word| token.is_word(word)))
    {
        temporary = temporary || TEMPORARY.iter().any(|word| token.is_word(word));
        cursor.next_token();
    }
    if robust || !cursor.eat("table") || cursor.eat_sequence(&["if", "not", "exists"]) {
        return false;
    }
    let dropped_on_commit = cursor
        .rest()
        .windows(3)
        .any(|words| matches!(words, [on, commit, drop] if on.is_word("on") && commit.is_word("commit") && drop.is_word("drop")));
    !(temporary && dropped_on_commit)
}
