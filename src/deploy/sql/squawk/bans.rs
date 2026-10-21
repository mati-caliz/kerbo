use super::statement::{Transaction, alter_table, transaction, violation, words};
use super::types::column_types;
use crate::deploy::Violation;
use crate::deploy::sql::columns::TypeKind;
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const CHAR_TYPES: &[&str] = &["char", "character", "bpchar"];
const DROPPED: &[(&[&str], MigrationRule)] = &[
    (&["drop", "database"], MigrationRule::BanDropDatabase),
    (&["drop", "schema"], MigrationRule::BanDropSchema),
    (&["drop", "domain"], MigrationRule::BanDropDomain),
    (&["drop", "extension"], MigrationRule::BanDropExtension),
];
const SCHEMA_MOVABLE: &[&[&str]] = &[
    &["alter", "view"],
    &["alter", "materialized", "view"],
    &["alter", "aggregate"],
    &["alter", "extension"],
    &["alter", "procedure"],
    &["alter", "routine"],
    &["alter", "function"],
    &["alter", "type"],
    &["alter", "sequence"],
    &["alter", "domain"],
];
const LONGEST_PREFIX: usize = 3;
const DOMAIN_CONSTRAINTS: &[&str] = &["constraint", "not", "null", "check", "default"];
const REINDEX_TARGETS: &[&str] = &["index", "table", "schema", "database"];

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        let leading = words(statement, LONGEST_PREFIX);
        if let (Some((_, rule)), Some(first)) = (
            DROPPED.iter().find(|(form, _)| leading.starts_with(form)),
            statement.first(),
        ) {
            found.push(violation(*rule, first, ""));
        }
        char_fields(statement, found);
        truncate_cascade(statement, found);
        set_schema(statement, &leading, found);
        table_actions(statement, found);
        domains(statement, found);
        reindex(statement, found);
    }
    uncommitted(statements, found);
}

fn table_actions(statement: &[Token], found: &mut Vec<Violation>) {
    let Some(altered) = alter_table(statement).filter(|altered| !altered.foreign) else {
        return;
    };
    for action in &altered.actions {
        column_options(action, found);
        if let Some(detach) = detach_without_concurrently(action) {
            found.push(violation(
                MigrationRule::RequireConcurrentPartitionDetach,
                detach,
                "",
            ));
        }
    }
}

fn char_fields(statement: &[Token], found: &mut Vec<Violation>) {
    for typ in column_types(statement) {
        let inner = if let TypeKind::Array(inner) = &typ.kind {
            inner.as_ref()
        } else {
            &typ.kind
        };
        let banned = match inner {
            TypeKind::Character => true,
            TypeKind::Path { name, .. } => CHAR_TYPES.contains(&name.to_ascii_lowercase().as_str()),
            TypeKind::Varchar { .. }
            | TypeKind::Bit
            | TypeKind::Double
            | TypeKind::Time
            | TypeKind::Timestamp { .. }
            | TypeKind::Interval
            | TypeKind::Array(_) => false,
        };
        if banned {
            found.push(violation(MigrationRule::BanCharField, typ.start, ""));
        }
    }
}

fn truncate_cascade(statement: &[Token], found: &mut Vec<Violation>) {
    if !statement.first().is_some_and(|first| first.is_word("truncate")) {
        return;
    }
    if let Some(cascade) = statement.last().filter(|last| last.is_word("cascade")) {
        found.push(violation(MigrationRule::BanTruncateCascade, cascade, ""));
    }
}

fn set_schema(statement: &[Token], leading: &[&str], found: &mut Vec<Violation>) {
    let movable =
        alter_table(statement).is_some() || SCHEMA_MOVABLE.iter().any(|form| leading.starts_with(form));
    if !movable {
        return;
    }
    let set = statement
        .windows(2)
        .find(|pair| matches!(pair, [set, schema] if set.is_word("set") && schema.is_word("schema")));
    if let Some([set, _]) = set {
        found.push(violation(MigrationRule::BanSetSchema, set, ""));
    }
}

fn column_options(action: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(action);
    if !cursor.eat("alter") {
        return;
    }
    cursor.eat("column");
    cursor.next_token();
    let options = cursor.rest();
    let Some(first) = options.first() else {
        return;
    };
    let mut option = Cursor::new(options);
    if option.eat_sequence(&["drop", "expression"]) {
        found.push(violation(MigrationRule::BanDropGeneratedExpression, first, ""));
    } else if option.eat_sequence(&["add", "generated"]) || option.eat_sequence(&["drop", "identity"]) {
        found.push(violation(MigrationRule::BanAlterIdentity, first, ""));
    } else if let Some([set, ..]) = options
        .windows(3)
        .find(|words| matches!(words, [set, generated, always] if set.is_word("set") && generated.is_word("generated") && always.is_word("always")))
    {
        found.push(violation(MigrationRule::BanAlterIdentity, set, ""));
    }
}

fn domains(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    if cursor.eat_sequence(&["create", "domain"]) {
        let constraint = cursor
            .rest()
            .iter()
            .find(|token| DOMAIN_CONSTRAINTS.iter().any(|word| token.is_word(word)));
        if let Some(constraint) = constraint {
            found.push(violation(
                MigrationRule::BanCreateDomainWithConstraint,
                constraint,
                "",
            ));
        }
        return;
    }
    if !cursor.eat_sequence(&["alter", "domain"]) {
        return;
    }
    cursor.qualified_name();
    if let Some(add) = cursor.peek().filter(|token| token.is_word("add")) {
        found.push(violation(MigrationRule::BanAlterDomainWithAddConstraint, add, ""));
    }
}

fn reindex(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    let Some(first) = statement.first().filter(|_| cursor.eat("reindex")) else {
        return;
    };
    let mut concurrently = false;
    if cursor.peek().is_some_and(|token| token.is_symbol('(')) {
        while let Some(token) = cursor.next_token() {
            concurrently = concurrently || token.is_word("concurrently");
            if token.is_symbol(')') {
                break;
            }
        }
    }
    if !cursor.eat_any(REINDEX_TARGETS) {
        return;
    }
    concurrently = concurrently || cursor.eat("concurrently");
    if !concurrently {
        found.push(violation(MigrationRule::RequireConcurrentReindex, first, ""));
    }
}

fn uncommitted(statements: &[&[Token]], found: &mut Vec<Violation>) {
    let mut open = None;
    for statement in statements {
        match transaction(statement) {
            Transaction::Begin => open = statement.first(),
            Transaction::Commit { .. } | Transaction::Rollback { .. } => open = None,
            Transaction::Other => {}
        }
    }
    if let Some(begin) = open {
        found.push(violation(MigrationRule::BanUncommittedTransaction, begin, ""));
    }
}

fn detach_without_concurrently(action: &[Token]) -> Option<&Token> {
    let mut cursor = Cursor::new(action);
    let detach = action.first()?;
    if !cursor.eat_sequence(&["detach", "partition"]) {
        return None;
    }
    cursor.qualified_name();
    (!cursor.eat_any(&["concurrently", "finalize"])).then_some(detach)
}
