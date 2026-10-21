use std::collections::HashSet;

use crate::deploy::Violation;
use crate::deploy::sql::cursor::{Cursor, normalized, top_level_items};
use crate::deploy::sql::lexer::Token;
use crate::rules::{MigrationRule, RuleId};

const TABLE_QUALIFIERS: &[&str] = &["global", "local", "temp", "temporary", "unlogged"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transaction {
    Begin,
    Commit { chain: bool, ends_transaction: bool },
    Rollback { chain: bool, ends_transaction: bool },
    Other,
}

pub fn transaction(tokens: &[Token]) -> Transaction {
    let mut cursor = Cursor::new(tokens);
    if cursor.eat("begin") || cursor.eat_sequence(&["start", "transaction"]) {
        return Transaction::Begin;
    }
    let commit = cursor.eat_any(&["commit", "end"]);
    let rollback = !commit && cursor.eat_any(&["rollback", "abort"]);
    if !commit && !rollback {
        return Transaction::Other;
    }
    let ends_transaction = !cursor
        .peek()
        .is_some_and(|token| token.is_word("to") || token.is_word("prepared"));
    cursor.eat_any(&["work", "transaction"]);
    let chain = cursor.eat_sequence(&["and", "chain"]);
    if commit {
        Transaction::Commit {
            chain,
            ends_transaction,
        }
    } else {
        Transaction::Rollback {
            chain,
            ends_transaction,
        }
    }
}

#[derive(Debug)]
pub struct CreatedTables {
    names: HashSet<Vec<String>>,
    inside_transaction: bool,
}

impl CreatedTables {
    pub fn new() -> Self {
        Self {
            names: HashSet::new(),
            inside_transaction: true,
        }
    }

    pub fn update(&mut self, tokens: &[Token]) {
        match transaction(tokens) {
            Transaction::Begin => self.inside_transaction = true,
            Transaction::Commit {
                chain,
                ends_transaction: true,
            }
            | Transaction::Rollback {
                chain,
                ends_transaction: true,
            } => {
                self.names.clear();
                self.inside_transaction = chain;
            }
            Transaction::Commit { .. } | Transaction::Rollback { .. } | Transaction::Other => {
                if let Some(name) = created_table(tokens).filter(|_| self.inside_transaction) {
                    self.names.insert(name);
                }
            }
        }
    }

    pub fn contains(&self, name: &[String]) -> bool {
        self.names.contains(name)
    }
}

fn created_table(tokens: &[Token]) -> Option<Vec<String>> {
    let mut cursor = Cursor::new(tokens);
    if cursor.eat("select") {
        let into = tokens.iter().position(|token| token.is_word("into"))?;
        let mut target = Cursor::new(tokens.get(into.saturating_add(1)..)?);
        while target.eat_any(TABLE_QUALIFIERS) {}
        target.eat("table");
        return Some(normalized(&target.qualified_name())).filter(|name| !name.is_empty());
    }
    if !cursor.eat("create") {
        return None;
    }
    cursor.eat_sequence(&["or", "replace"]);
    while cursor.eat_any(TABLE_QUALIFIERS) {}
    if !cursor.eat("table") {
        return None;
    }
    cursor.eat_sequence(&["if", "not", "exists"]);
    Some(normalized(&cursor.qualified_name())).filter(|name| !name.is_empty())
}

#[derive(Debug)]
pub struct AlterTable<'tokens> {
    pub foreign: bool,
    pub name: Vec<String>,
    pub actions: Vec<&'tokens [Token]>,
}

pub fn alter_table(tokens: &[Token]) -> Option<AlterTable<'_>> {
    let mut cursor = Cursor::new(tokens);
    if !cursor.eat("alter") {
        return None;
    }
    let foreign = cursor.eat_sequence(&["foreign", "table"]);
    if !foreign && !cursor.eat("table") {
        return None;
    }
    cursor.eat_sequence(&["if", "exists"]);
    cursor.eat("only");
    let name = normalized(&cursor.qualified_name());
    let rest = cursor.rest();
    let rest = rest
        .split_first()
        .filter(|(first, _)| first.is_symbol('*'))
        .map_or(rest, |(_, after)| after);
    Some(AlterTable {
        foreign,
        name,
        actions: top_level_items(rest),
    })
}

pub fn violation(rule: MigrationRule, token: &Token, subject: &str) -> Violation {
    Violation {
        rule: RuleId::Migration(rule),
        line: token.line,
        column: token.column,
        subject: subject.to_owned(),
        fix: None,
    }
}

pub fn words(tokens: &[Token], count: usize) -> Vec<&str> {
    tokens.iter().take(count).filter_map(Token::word).collect()
}
