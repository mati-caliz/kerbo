mod alter;
mod columns;
mod cursor;
mod ignore;
mod lexer;
mod squawk;

use std::collections::{HashMap, HashSet};

use cursor::{Cursor, normalized, statements};
use lexer::{Token, tokenize};

use super::Violation;
use crate::rules::{MigrationRule, RuleId};

const TABLE_QUALIFIERS: &[&str] = &["global", "local", "temp", "temporary", "unlogged"];
const NAME_SEPARATOR: &str = ", ";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TableColumn {
    table: String,
    column: String,
}

#[derive(Debug)]
struct Migration {
    created_tables: HashSet<Vec<String>>,
    inside_transaction: bool,
    not_null_checks: HashMap<String, TableColumn>,
    validated_columns: HashSet<TableColumn>,
    externally_validated_tables: HashSet<String>,
    violations: Vec<Violation>,
}

pub fn violations(source: &str, first_line: usize) -> Vec<Violation> {
    let tokens = tokenize(source, first_line);
    let statements = statements(&tokens);
    let mut found = migration_violations(&statements);
    found.extend(squawk::violations(&statements));
    found.extend(unknown_ignored_rules(source));
    found.sort_by_key(|violation| (violation.line, violation.column));
    found
}

pub use ignore::squawk_ignores;

fn unknown_ignored_rules(source: &str) -> Vec<Violation> {
    squawk_ignores(source)
        .into_iter()
        .flat_map(|ignore| ignore.unknown)
        .map(|unknown| Violation {
            rule: RuleId::Migration(MigrationRule::UnusedIgnore),
            line: unknown.line,
            column: unknown.column,
            subject: unknown.name,
            fix: None,
        })
        .collect()
}

pub fn snippet_violations(source: &str, first_line: usize) -> Vec<Violation> {
    let tokens = tokenize(source, first_line);
    migration_violations(&statements(&tokens))
}

fn migration_violations(statements: &[&[Token]]) -> Vec<Violation> {
    let mut migration = Migration {
        created_tables: HashSet::new(),
        inside_transaction: true,
        not_null_checks: HashMap::new(),
        validated_columns: HashSet::new(),
        externally_validated_tables: HashSet::new(),
        violations: Vec::new(),
    };
    for statement in statements {
        migration.statement(statement);
    }
    migration.violations
}

impl Migration {
    fn report(&mut self, rule: MigrationRule, token: &Token, subject: String) {
        self.violations.push(Violation {
            rule: RuleId::Migration(rule),
            line: token.line,
            column: token.column,
            subject,
            fix: None,
        });
    }

    fn statement(&mut self, statement: &[Token]) {
        let Some(first) = statement.first() else {
            return;
        };
        let mut cursor = Cursor::new(statement);
        if cursor.eat("create") {
            self.create(first, cursor);
        } else if cursor.eat("drop") {
            self.drop(first, cursor);
        } else if cursor.eat("alter") {
            alter::alter(self, cursor);
        } else if cursor.eat("begin") || cursor.eat_sequence(&["start", "transaction"]) {
            self.inside_transaction = true;
        } else if cursor.eat_any(&["commit", "end", "rollback"]) {
            self.finish_transaction(cursor);
        }
    }

    fn finish_transaction(&mut self, mut cursor: Cursor<'_>) {
        if cursor
            .peek()
            .is_some_and(|token| token.is_word("to") || token.is_word("prepared"))
        {
            return;
        }
        cursor.eat_any(&["work", "transaction"]);
        self.created_tables.clear();
        self.inside_transaction = cursor.eat_sequence(&["and", "chain"]);
    }

    fn create(&mut self, first: &Token, mut cursor: Cursor<'_>) {
        cursor.eat_sequence(&["or", "replace"]);
        cursor.eat("unique");
        if cursor.eat("index") {
            self.create_index(first, cursor);
            return;
        }
        while cursor.eat_any(TABLE_QUALIFIERS) {}
        if cursor.eat("table") && self.inside_transaction {
            cursor.eat_sequence(&["if", "not", "exists"]);
            let name = normalized(&cursor.qualified_name());
            if !name.is_empty() {
                self.created_tables.insert(name);
            }
        }
    }

    fn create_index(&mut self, first: &Token, mut cursor: Cursor<'_>) {
        if cursor.eat("concurrently") {
            return;
        }
        while cursor.peek().is_some_and(|token| !token.is_word("on")) {
            cursor.next_token();
        }
        cursor.eat("on");
        cursor.eat("only");
        let table = normalized(&cursor.qualified_name());
        if table.is_empty() || self.created_tables.contains(&table) {
            return;
        }
        self.report(
            MigrationRule::RequireConcurrentIndexCreation,
            first,
            table.join("."),
        );
    }

    fn drop(&mut self, first: &Token, mut cursor: Cursor<'_>) {
        if cursor.eat("index") {
            if !cursor.eat("concurrently") {
                cursor.eat_sequence(&["if", "exists"]);
                let name = normalized(&cursor.qualified_name()).join(".");
                self.report(MigrationRule::RequireConcurrentIndexDeletion, first, name);
            }
            return;
        }
        cursor.eat("foreign");
        if cursor.eat("table") {
            cursor.eat_sequence(&["if", "exists"]);
            let names: Vec<String> = cursor::top_level_items(cursor.rest())
                .into_iter()
                .map(|item| normalized(&Cursor::new(item).qualified_name()).join("."))
                .collect();
            self.report(MigrationRule::BanDropTable, first, names.join(NAME_SEPARATOR));
        }
    }
}

#[cfg(test)]
mod tests;
