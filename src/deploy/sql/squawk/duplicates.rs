use std::collections::HashMap;

use super::names::target_name;
use super::statement::violation;
use super::types::create_table_arguments;
use crate::deploy::Violation;
use crate::deploy::sql::cursor::{Cursor, top_level_items};
use crate::deploy::sql::lexer::Token;
use crate::rules::MigrationRule;

const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const QUALIFIER_SEPARATOR: char = '.';
const OPEN_BRACKET: char = '[';
const ASSIGNMENT: char = '=';
const TABLE_CONSTRAINT_STARTS: &[&str] = &[
    "constraint",
    "primary",
    "unique",
    "foreign",
    "check",
    "exclude",
    "like",
];
const VIEW_QUALIFIERS: &[&str] = &["or", "replace", "temp", "temporary", "recursive", "materialized"];
const SELECT_ENDS: &[&str] = &[
    "from",
    "into",
    "where",
    "group",
    "having",
    "window",
    "order",
    "limit",
    "offset",
    "union",
    "intersect",
    "except",
    "fetch",
    "for",
];
const SET_ENDS: &[&str] = &["from", "where", "returning", "when"];
const COMPOUND_WORDS: &[&str] = &["union", "intersect", "except"];

struct Assigned<'tokens> {
    key: String,
    node: &'tokens Token,
    partial: bool,
}

pub fn violations(statements: &[&[Token]], found: &mut Vec<Violation>) {
    for statement in statements {
        if let Some(arguments) = create_table_arguments(statement) {
            report(&defined_columns(arguments), found);
        }
        for list in insert_column_lists(statement) {
            report(&assigned_columns(top_level_items(list)), found);
        }
        for clause in set_clauses(statement) {
            report(&assigned_columns(set_targets(clause)), found);
        }
        if let Some(defined) = view_columns(statement).or_else(|| select_into_columns(statement)) {
            report(&defined, found);
        }
    }
}

fn report(assigned: &[Assigned<'_>], found: &mut Vec<Violation>) {
    let mut groups: HashMap<&str, Vec<&Assigned<'_>>> = HashMap::new();
    for column in assigned {
        groups.entry(column.key.as_str()).or_default().push(column);
    }
    let mut repeated: Vec<&Assigned<'_>> = groups
        .into_values()
        .filter(|group| group.len() > 1 && !group.iter().all(|column| column.partial))
        .flatten()
        .collect();
    repeated.sort_by_key(|column| (column.node.line, column.node.column));
    found.extend(repeated.into_iter().map(|column| {
        violation(
            MigrationRule::BanDuplicateColumnAssignments,
            column.node,
            &column.key,
        )
    }));
}

fn defined_columns(arguments: &[Token]) -> Vec<Assigned<'_>> {
    top_level_items(arguments)
        .into_iter()
        .filter(|item| {
            item.first()
                .is_some_and(|first| !TABLE_CONSTRAINT_STARTS.iter().any(|word| first.is_word(word)))
        })
        .filter_map(|item| item.first())
        .filter_map(|name| {
            Some(Assigned {
                key: name.identifier()?,
                node: name,
                partial: false,
            })
        })
        .collect()
}

fn assigned_columns(targets: Vec<&[Token]>) -> Vec<Assigned<'_>> {
    targets
        .into_iter()
        .filter_map(|target| {
            let name = target.first()?;
            let partial = target
                .get(1)
                .is_some_and(|next| next.is_symbol(QUALIFIER_SEPARATOR) || next.is_symbol(OPEN_BRACKET));
            Some(Assigned {
                key: name.identifier()?,
                node: name,
                partial,
            })
        })
        .collect()
}

fn insert_column_lists(statement: &[Token]) -> Vec<&[Token]> {
    statement
        .windows(2)
        .enumerate()
        .filter(
            |(_, pair)| matches!(pair, [insert, into] if insert.is_word("insert") && into.is_word("into")),
        )
        .filter_map(|(index, _)| {
            let mut cursor = Cursor::new(statement.get(index.saturating_add(2)..)?);
            cursor.qualified_name();
            if cursor.eat("as") {
                cursor.next_token();
            }
            parenthesized(cursor.rest())
        })
        .chain(merge_insert_lists(statement))
        .collect()
}

fn merge_insert_lists(statement: &[Token]) -> Vec<&[Token]> {
    if !statement.first().is_some_and(|first| first.is_word("merge")) {
        return Vec::new();
    }
    statement
        .windows(2)
        .enumerate()
        .filter(
            |(_, pair)| matches!(pair, [then, insert] if then.is_word("then") && insert.is_word("insert")),
        )
        .filter_map(|(index, _)| parenthesized(statement.get(index.saturating_add(2)..)?))
        .collect()
}

fn parenthesized(tokens: &[Token]) -> Option<&[Token]> {
    if !tokens.first()?.is_symbol(OPEN_PARENTHESIS) {
        return None;
    }
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return tokens.get(1..index);
            }
        }
    }
    None
}

fn set_clauses(statement: &[Token]) -> Vec<&[Token]> {
    let mut starts: Vec<usize> = statement
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| matches!(pair, [update, set] if update.is_word("update") && set.is_word("set")))
        .map(|(index, _)| index.saturating_add(1))
        .collect();
    if statement.first().is_some_and(|first| first.is_word("update"))
        && let Some(set) = top_level_position(statement, |token| token.is_word("set"))
    {
        starts.push(set);
    }
    starts.sort_unstable();
    starts.dedup();
    starts
        .into_iter()
        .filter_map(|index| {
            let rest = statement.get(index.saturating_add(1)..)?;
            let end = top_level_position(rest, |token| is_any_word(token, SET_ENDS)).unwrap_or(rest.len());
            rest.get(..end)
        })
        .collect()
}

fn set_targets(clause: &[Token]) -> Vec<&[Token]> {
    top_level_items(clause)
        .into_iter()
        .flat_map(|item| {
            let target_end = item
                .iter()
                .position(|token| token.is_symbol(ASSIGNMENT))
                .unwrap_or(item.len());
            let target = item.get(..target_end).unwrap_or_default();
            parenthesized(target).map_or_else(|| vec![target], top_level_items)
        })
        .collect()
}

fn view_columns(statement: &[Token]) -> Option<Vec<Assigned<'_>>> {
    let mut cursor = Cursor::new(statement);
    if !cursor.eat("create") {
        return None;
    }
    while cursor.eat_any(VIEW_QUALIFIERS) {}
    if !cursor.eat("view") {
        return None;
    }
    cursor.eat_sequence(&["if", "not", "exists"]);
    cursor.qualified_name();
    if let Some(list) = parenthesized(cursor.rest()) {
        return Some(assigned_columns(top_level_items(list)));
    }
    let start = cursor.rest().iter().position(|token| token.is_word("as"))?;
    let query = cursor.rest().get(start.saturating_add(1)..)?;
    Some(inferred(select_targets(query)))
}

fn select_into_columns(statement: &[Token]) -> Option<Vec<Assigned<'_>>> {
    if !statement.first()?.is_word("select") {
        return None;
    }
    let into = top_level_position(statement, |token| token.is_word("into"))?;
    Some(inferred(select_targets(statement.get(..into)?)))
}

fn inferred(targets: Vec<&[Token]>) -> Vec<Assigned<'_>> {
    targets
        .into_iter()
        .filter_map(target_name)
        .map(|(name, node)| Assigned {
            key: name.text(),
            node,
            partial: false,
        })
        .collect()
}

pub fn select_targets(query: &[Token]) -> Vec<&[Token]> {
    if top_level_position(query, |token| is_any_word(token, COMPOUND_WORDS)).is_some() {
        return Vec::new();
    }
    let Some(select) = top_level_position(query, |token| token.is_word("select")) else {
        return Vec::new();
    };
    let mut cursor = Cursor::new(query.get(select.saturating_add(1)..).unwrap_or_default());
    cursor.eat("all");
    if cursor.eat("distinct") && cursor.eat("on") {
        let skipped = parenthesized(cursor.rest()).map_or(0, |inside| inside.len().saturating_add(2));
        cursor = Cursor::new(cursor.rest().get(skipped..).unwrap_or_default());
    }
    let rest = cursor.rest();
    let end = top_level_position(rest, |token| is_any_word(token, SELECT_ENDS)).unwrap_or(rest.len());
    top_level_items(rest.get(..end).unwrap_or_default())
}

fn top_level_position(tokens: &[Token], wanted: impl Fn(&Token) -> bool) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && wanted(token) {
            return Some(index);
        }
    }
    None
}

fn is_any_word(token: &Token, words: &[&str]) -> bool {
    words.iter().any(|word| token.is_word(word))
}
