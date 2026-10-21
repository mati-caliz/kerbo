use super::statement::{alter_table, violation, words};
use crate::deploy::Violation;
use crate::deploy::sql::cursor::Cursor;
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const DROPPED_TYPES: &[&[&str]] = &[
    &["drop", "type"],
    &["drop", "operator", "class"],
    &["drop", "operator", "family"],
    &["drop", "operator"],
    &["drop", "cast"],
];
const DROPPED_FUNCTIONS: &[&[&str]] = &[
    &["drop", "aggregate"],
    &["drop", "function"],
    &["drop", "procedure"],
    &["drop", "routine"],
];
const DROPPED_SEQUENCES: &[&[&str]] = &[&["drop", "sequence"]];
const DROPPED_VIEWS: &[&[&str]] = &[&["drop", "view"], &["drop", "materialized", "view"]];
const LONGEST_DROP: usize = 3;
const RENAMED_OBJECTS: &[&str] = &[
    "role",
    "user",
    "group",
    "database",
    "trigger",
    "policy",
    "routine",
    "view",
    "function",
    "aggregate",
    "procedure",
    "sequence",
    "schema",
    "index",
];
const RENAMED_TWO_WORD_OBJECTS: &[&[&str]] = &[&["materialized", "view"]];

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        dropped_object(statement, found);
        if let Some(altered) = alter_table(statement) {
            table_actions(altered.foreign, &altered.actions, found);
            continue;
        }
        renamed_object(statement, found);
        altered_type(statement, found);
        altered_domain(statement, found);
        altered_view_column(statement, found);
    }
}

fn dropped_object(statement: &[Token], found: &mut Vec<Violation>) {
    let leading = words(statement, LONGEST_DROP);
    let rules = [
        (MigrationRule::BanDropType, DROPPED_TYPES),
        (MigrationRule::BanDropFunction, DROPPED_FUNCTIONS),
        (MigrationRule::BanDropSequence, DROPPED_SEQUENCES),
        (MigrationRule::BanDropView, DROPPED_VIEWS),
    ];
    let matched = rules
        .into_iter()
        .find(|(_, forms)| forms.iter().any(|form| leading.starts_with(form)));
    if let (Some((rule, _)), Some(first)) = (matched, statement.first()) {
        found.push(violation(rule, first, ""));
    }
}

fn table_actions(foreign: bool, actions: &[&[Token]], found: &mut Vec<Violation>) {
    for action in actions {
        let mut cursor = Cursor::new(action);
        if cursor.eat_sequence(&["rename", "to"]) {
            if foreign && let Some(first) = action.first() {
                found.push(violation(MigrationRule::RenamingObject, first, ""));
            }
            continue;
        }
        if cursor.eat_sequence(&["rename", "constraint"]) {
            if !foreign && let Some(first) = action.first() {
                found.push(violation(MigrationRule::RenamingObject, first, ""));
            }
            continue;
        }
        if !cursor.eat("alter") {
            continue;
        }
        cursor.eat("column");
        cursor.next_token();
        let Some(option) = cursor.peek() else {
            continue;
        };
        if cursor.eat_sequence(&["drop", "default"]) {
            found.push(violation(MigrationRule::BanDropDefault, option, ""));
        } else if cursor.eat_sequence(&["drop", "not", "null"]) {
            found.push(violation(MigrationRule::BanDropNotNull, option, ""));
        }
    }
}

fn renamed_object(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat("alter") {
        return;
    }
    let known = RENAMED_TWO_WORD_OBJECTS
        .iter()
        .any(|object| cursor.eat_sequence(object))
        || RENAMED_OBJECTS.iter().any(|object| cursor.eat(object));
    if !known {
        return;
    }
    let rename = statement
        .windows(2)
        .find(|pair| matches!(pair, [rename, to] if rename.is_word("rename") && to.is_word("to")));
    if let Some([rename, _]) = rename {
        found.push(violation(MigrationRule::RenamingObject, rename, ""));
    }
}

fn altered_type(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat_sequence(&["alter", "type"]) {
        return;
    }
    cursor.qualified_name();
    let Some(first) = cursor.peek() else {
        return;
    };
    if cursor.eat_sequence(&["rename", "to"]) || cursor.eat_sequence(&["rename", "value"]) {
        found.push(violation(MigrationRule::RenamingObject, first, ""));
        return;
    }
    if !cursor.eat_sequence(&["add", "value"]) {
        return;
    }
    cursor.eat_sequence(&["if", "not", "exists"]);
    cursor.next_token();
    if !cursor
        .peek()
        .is_some_and(|token| token.is_word("before") || token.is_word("after"))
    {
        found.push(violation(MigrationRule::RequireEnumValueOrdering, first, ""));
    }
}

fn altered_domain(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat_sequence(&["alter", "domain"]) {
        return;
    }
    cursor.qualified_name();
    let Some(first) = cursor.peek() else {
        return;
    };
    let rule = if cursor.eat_sequence(&["drop", "default"]) {
        MigrationRule::BanDropDefault
    } else if cursor.eat_sequence(&["drop", "not", "null"]) {
        MigrationRule::BanDropNotNull
    } else if cursor.eat_sequence(&["rename", "to"]) || cursor.eat_sequence(&["rename", "constraint"]) {
        MigrationRule::RenamingObject
    } else {
        return;
    };
    found.push(violation(rule, first, ""));
}

fn altered_view_column(statement: &[Token], found: &mut Vec<Violation>) {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat_sequence(&["alter", "view"]) {
        return;
    }
    cursor.eat_sequence(&["if", "exists"]);
    cursor.qualified_name();
    if !cursor.eat("alter") {
        return;
    }
    cursor.eat("column");
    cursor.next_token();
    if let Some(first) = cursor.peek()
        && cursor.eat_sequence(&["drop", "default"])
    {
        found.push(violation(MigrationRule::BanDropDefault, first, ""));
    }
}
