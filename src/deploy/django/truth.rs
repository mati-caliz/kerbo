use tree_sitter::Node;

use super::settings::Settings;
use super::values::{boolean, parenthesized_content, text};
use crate::syntax::named_children;

const SETTINGS_OBJECT: &str = "settings";
const NEGATION: &str = "not_operator";
const ATTRIBUTE: &str = "attribute";
const COMPARISON: &str = "comparison_operator";
const DATABASE_VENDOR: &str = "connection.vendor";
const POSTGRES_VENDOR: &str = "postgresql";
const EQUAL: &str = "==";
const NOT_EQUAL: &str = "!=";
const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const ELSE: &str = "else_clause";

pub enum Branch<'tree> {
    Taken(Node<'tree>),
    Skipped,
}

pub fn truth(node: Node<'_>, source: &[u8], settings: &Settings) -> Option<bool> {
    if let Some(content) = parenthesized_content(node) {
        return truth(content, source, settings);
    }
    match node.kind() {
        NEGATION => truth(node.child_by_field_name("argument")?, source, settings).map(|value| !value),
        ATTRIBUTE => {
            let object = node.child_by_field_name("object")?.utf8_text(source).ok()?;
            let name = node.child_by_field_name("attribute")?.utf8_text(source).ok()?;
            (object == SETTINGS_OBJECT).then(|| settings.flag(name)).flatten()
        }
        COMPARISON => vendor_comparison(node, source),
        _ => boolean(node),
    }
}

fn vendor_comparison(node: Node<'_>, source: &[u8]) -> Option<bool> {
    let [left, right] = named_children(node).try_into().ok()?;
    let mut cursor = node.walk();
    let operator = node
        .children_by_field_name("operators", &mut cursor)
        .next()?
        .utf8_text(source)
        .ok()?;
    let vendor = [(left, right), (right, left)]
        .into_iter()
        .find(|(subject, _)| subject.utf8_text(source).ok() == Some(DATABASE_VENDOR))
        .and_then(|(_, literal)| text(literal, source))?;
    let is_postgres = vendor == POSTGRES_VENDOR;
    match operator {
        EQUAL => Some(is_postgres),
        NOT_EQUAL => Some(!is_postgres),
        _ => None,
    }
}

pub fn taken_branch<'tree>(
    statement: Node<'tree>,
    source: &[u8],
    settings: &Settings,
) -> Option<Branch<'tree>> {
    if truth(statement.child_by_field_name("condition")?, source, settings)? {
        return statement.child_by_field_name("consequence").map(Branch::Taken);
    }
    let mut cursor = statement.walk();
    let alternatives: Vec<Node<'tree>> = statement
        .children_by_field_name("alternative", &mut cursor)
        .collect();
    for alternative in alternatives {
        match alternative.kind() {
            ELIF if truth(alternative.child_by_field_name("condition")?, source, settings)? => {
                return alternative.child_by_field_name("consequence").map(Branch::Taken);
            }
            ELSE => return alternative.child_by_field_name("body").map(Branch::Taken),
            _ => {}
        }
    }
    Some(Branch::Skipped)
}

pub fn reachable_statements<'tree>(
    block: Node<'tree>,
    source: &[u8],
    settings: &Settings,
) -> Vec<Node<'tree>> {
    named_children(block)
        .into_iter()
        .flat_map(|statement| {
            if statement.kind() != IF {
                return vec![statement];
            }
            match taken_branch(statement, source, settings) {
                Some(Branch::Taken(branch)) => reachable_statements(branch, source, settings),
                Some(Branch::Skipped) | None => Vec::new(),
            }
        })
        .collect()
}
