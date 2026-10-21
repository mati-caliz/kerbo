use std::collections::HashSet;

use super::statement::{AlterTable, CreatedTables, Transaction, alter_table, transaction, violation};
use super::volatility::is_non_volatile;
use crate::deploy::Violation;
use crate::deploy::sql::columns::{Constraint, ConstraintKind, column, constraint};
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const TABLE_CONSTRAINT_STARTS: &[&str] = &["constraint", "primary", "unique", "foreign", "check", "exclude"];

enum Added<'tokens> {
    Constraint(Constraint<'tokens>),
    Column(Vec<Constraint<'tokens>>),
}

fn added(action: &[Token]) -> Option<Added<'_>> {
    let mut cursor = Cursor::new(action);
    if !cursor.eat("add") {
        return None;
    }
    if cursor
        .peek()
        .is_some_and(|token| TABLE_CONSTRAINT_STARTS.iter().any(|word| token.is_word(word)))
    {
        return constraint(cursor.rest()).map(Added::Constraint);
    }
    cursor.eat("column");
    cursor.eat_sequence(&["if", "not", "exists"]);
    cursor.next_token();
    Some(Added::Column(column(cursor.rest()).constraints))
}

const fn is_foreign_key(kind: ConstraintKind) -> bool {
    matches!(kind, ConstraintKind::ForeignKey | ConstraintKind::References)
}

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    validated_in_transaction(statements, found);
    let mut created = CreatedTables::new();
    for statement in statements {
        created.update(statement);
        let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) else {
            continue;
        };
        let new_table = created.contains(&altered.name);
        for action in &altered.actions {
            match added(action) {
                Some(Added::Constraint(added)) => added_constraint(action, &added, new_table, found),
                Some(Added::Column(constraints)) => added_column(&constraints, new_table, found),
                None => {}
            }
        }
    }
    dropped_constraints(statements, found);
    defaults(statements, found);
}

fn added_constraint(action: &[Token], added: &Constraint<'_>, new_table: bool, found: &mut Vec<Violation>) {
    let unvalidated = !added.is_not_valid();
    let indexed =
        matches!(added.kind, ConstraintKind::Unique | ConstraintKind::PrimaryKey) && added.uses_index();
    if let Some(first) = action.first().filter(|_| unvalidated && !new_table && !indexed) {
        found.push(violation(MigrationRule::ConstraintMissingNotValid, first, ""));
    }
    if unvalidated && !new_table && is_foreign_key(added.kind) {
        found.push(violation(
            MigrationRule::AddingForeignKeyConstraint,
            added.start,
            "",
        ));
    }
    if !new_table && added.kind == ConstraintKind::PrimaryKey && !added.uses_index() {
        found.push(violation(
            MigrationRule::AddingSerialPrimaryKeyField,
            added.start,
            "",
        ));
    }
    if !new_table && added.kind == ConstraintKind::Unique && !added.uses_index() {
        found.push(violation(
            MigrationRule::DisallowedUniqueConstraint,
            added.start,
            "",
        ));
    }
}

fn added_column(constraints: &[Constraint<'_>], new_table: bool, found: &mut Vec<Violation>) {
    for added in constraints {
        let rule = match added.kind {
            ConstraintKind::References | ConstraintKind::ForeignKey => {
                MigrationRule::AddingForeignKeyConstraint
            }
            ConstraintKind::PrimaryKey if !new_table && !added.uses_index() => {
                MigrationRule::AddingSerialPrimaryKeyField
            }
            ConstraintKind::Unique => MigrationRule::DisallowedUniqueConstraint,
            ConstraintKind::PrimaryKey
            | ConstraintKind::Default
            | ConstraintKind::Generated
            | ConstraintKind::Check
            | ConstraintKind::Other => continue,
        };
        found.push(violation(rule, added.start, ""));
    }
}

fn validated_in_transaction(statements: &[&[Token]], found: &mut Vec<Violation>) {
    let mut inside_transaction = true;
    let mut not_valid = HashSet::new();
    for statement in statements {
        match transaction(statement) {
            Transaction::Begin => {
                if !inside_transaction {
                    not_valid.clear();
                }
                inside_transaction = true;
            }
            Transaction::Commit { .. } => inside_transaction = false,
            Transaction::Rollback { .. } | Transaction::Other => {}
        }
        let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) else {
            continue;
        };
        for action in &altered.actions {
            remember_or_report(action, inside_transaction, &mut not_valid, found);
        }
    }
}

fn remember_or_report(
    action: &[Token],
    inside_transaction: bool,
    not_valid: &mut HashSet<String>,
    found: &mut Vec<Violation>,
) {
    let mut cursor = Cursor::new(action);
    if cursor.eat_sequence(&["validate", "constraint"]) {
        let name = cursor.next_token().and_then(Token::identifier);
        if let (Some(name), Some(first)) = (name, action.first())
            && inside_transaction
            && not_valid.contains(&name)
        {
            found.push(violation(MigrationRule::ConstraintMissingNotValid, first, &name));
        }
        return;
    }
    if let Some(Added::Constraint(added)) = added(action)
        && added.is_not_valid()
        && let Some(name) = added.name
    {
        not_valid.insert(name);
    }
}

fn dropped_constraints(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        if let Some(altered) = alter_table(statement) {
            table_drops(&altered, found);
            continue;
        }
        let mut cursor = Cursor::new(statement);
        if cursor.eat_sequence(&["alter", "domain"]) {
            cursor.qualified_name();
            if let Some(first) = cursor.peek()
                && cursor.eat_sequence(&["drop", "constraint"])
            {
                found.push(violation(MigrationRule::BanDropConstraint, first, ""));
            }
        }
    }
}

fn table_drops(altered: &AlterTable<'_>, found: &mut Vec<Violation>) {
    for action in &altered.actions {
        let mut cursor = Cursor::new(action);
        if cursor.eat_sequence(&["drop", "constraint"]) {
            if let Some(first) = action.first() {
                found.push(violation(MigrationRule::BanDropConstraint, first, ""));
            }
            continue;
        }
        if cursor.eat_sequence(&["alter", "constraint"]) {
            let not_enforced = action.windows(2).find(
                |pair| matches!(pair, [not, enforced] if not.is_word("not") && enforced.is_word("enforced")),
            );
            if let Some([not, _]) = not_enforced {
                found.push(violation(MigrationRule::BanDropConstraint, not, ""));
            }
        }
    }
}

fn defaults(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) else {
            continue;
        };
        for action in &altered.actions {
            if let Some(Added::Column(constraints)) = added(action) {
                found.extend(constraints.iter().filter_map(rewriting_default));
            }
        }
    }
}

fn rewriting_default(added: &Constraint<'_>) -> Option<Violation> {
    let start = match added.kind {
        ConstraintKind::Default if !is_non_volatile(added.body) => added.body.first()?,
        ConstraintKind::Generated => added.start,
        ConstraintKind::Default
        | ConstraintKind::References
        | ConstraintKind::ForeignKey
        | ConstraintKind::Unique
        | ConstraintKind::PrimaryKey
        | ConstraintKind::Check
        | ConstraintKind::Other => return None,
    };
    Some(violation(MigrationRule::AddingFieldWithDefault, start, ""))
}
