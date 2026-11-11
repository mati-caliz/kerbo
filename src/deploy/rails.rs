mod arguments;
mod columns;
mod ddl;
mod references;
mod statements;
mod tables;

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use tree_sitter::Node;

use super::Violation;
use super::script::{Origin, Script};
use crate::syntax::{descendants, named_children, one_based};
use arguments::{Arguments, heredoc_body, heredoc_text, name, string_content};

pub const EXTENSION: &str = "rb";

const DATABASE_DIRECTORY: &str = "db";
const MIGRATE_DIRECTORY: &str = "migrate";
const CLASS: &str = "class";
const METHOD: &str = "method";
const SINGLETON_METHOD: &str = "singleton_method";
const CALL: &str = "call";
const IDENTIFIER: &str = "identifier";
const STRING: &str = "string";
const MIGRATION_MARKER: &str = "Migration";
const DISABLE_TRANSACTION: &str = "disable_ddl_transaction!";
const ENTRY_POINTS: &[&str] = &["change", "up"];
const WRAPPERS: &[&str] = &[
    "safety_assured",
    "reversible",
    "up_only",
    "say_with_time",
    "transaction",
    "revert",
];
const UP_DIRECTION: &str = "up";
const DOWN_DIRECTION: &str = "down";
const EXECUTE: &str = "execute";
const CREATE_TABLE: &str = "create_table";
const CHANGE_TABLE: &str = "change_table";
const SKIPPED_KINDS: &[&str] = &[
    "string",
    "heredoc_body",
    "lambda",
    "comment",
    "method",
    "singleton_method",
    "class",
];

pub fn is_migration(path: &Path) -> bool {
    let components: Vec<&str> = path
        .components()
        .filter_map(|component| {
            if let Component::Normal(part) = component {
                part.to_str()
            } else {
                None
            }
        })
        .collect();
    components
        .windows(2)
        .any(|pair| matches!(pair, [database, migrate] if *database == DATABASE_DIRECTORY && *migrate == MIGRATE_DIRECTORY))
}

pub fn violations(root: Node<'_>, source: &[u8]) -> Vec<Violation> {
    script(root, source).map(Script::finish).unwrap_or_default()
}

fn script(root: Node<'_>, source: &[u8]) -> Option<Script> {
    let class = migration_class(root, source)?;
    let transactional = !disables_transaction(class, source);
    let mut walker = Walker {
        source,
        helpers: helpers(class, source),
        visiting: HashSet::new(),
        script: if transactional {
            Script::new()
        } else {
            Script::without_transaction()
        },
    };
    for entry in ENTRY_POINTS {
        if let Some(body) = walker.helpers.get(*entry).copied() {
            walker.visiting.insert((*entry).to_owned());
            walker.walk(body);
            break;
        }
    }
    let mut script = walker.script;
    if transactional {
        script.leave_transaction();
    }
    Some(script)
}

fn migration_class<'tree>(root: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    descendants(root).into_iter().find(|node| {
        node.kind() == CLASS
            && node
                .child_by_field_name("superclass")
                .is_some_and(|superclass| arguments::raw(superclass, source).contains(MIGRATION_MARKER))
    })
}

fn disables_transaction(class: Node<'_>, source: &[u8]) -> bool {
    descendants(class)
        .into_iter()
        .any(|node| node.kind() == IDENTIFIER && arguments::raw(node, source) == DISABLE_TRANSACTION)
}

fn helpers<'tree>(class: Node<'tree>, source: &[u8]) -> HashMap<String, Node<'tree>> {
    let Some(body) = class.child_by_field_name("body") else {
        return HashMap::new();
    };
    named_children(body)
        .into_iter()
        .filter(|node| [METHOD, SINGLETON_METHOD].contains(&node.kind()))
        .filter_map(|method| {
            let method_name = arguments::raw(method.child_by_field_name("name")?, source);
            Some((method_name, method.child_by_field_name("body")?))
        })
        .collect()
}

struct Walker<'tree, 'source> {
    source: &'source [u8],
    helpers: HashMap<String, Node<'tree>>,
    visiting: HashSet<String>,
    script: Script,
}

impl<'tree> Walker<'tree, '_> {
    fn walk(&mut self, node: Node<'tree>) {
        match node.kind() {
            CALL => self.call(node),
            IDENTIFIER => self.helper(&arguments::raw(node, self.source)),
            kind if SKIPPED_KINDS.contains(&kind) => {}
            _ => named_children(node)
                .into_iter()
                .for_each(|child| self.walk(child)),
        }
    }

    fn helper(&mut self, method: &str) {
        let Some(body) = self.helpers.get(method).copied() else {
            return;
        };
        if !self.visiting.insert(method.to_owned()) {
            return;
        }
        self.walk(body);
        self.visiting.remove(method);
    }

    fn walk_block(&mut self, call: Node<'tree>) {
        if let Some(block) = call.child_by_field_name("block") {
            self.walk(block);
        }
    }

    fn call(&mut self, call: Node<'tree>) {
        let Some(method) = call
            .child_by_field_name("method")
            .map(|node| arguments::raw(node, self.source))
        else {
            return;
        };
        if call.child_by_field_name("receiver").is_some() {
            if method == UP_DIRECTION {
                self.walk_block(call);
            }
            return;
        }
        if method == DOWN_DIRECTION {
            return;
        }
        if WRAPPERS.contains(&method.as_str()) {
            self.walk_block(call);
            return;
        }
        if self.helpers.contains_key(&method) {
            self.helper(&method);
            return;
        }
        let list = call.child_by_field_name("arguments");
        if method == EXECUTE {
            self.execute(list);
            return;
        }
        let parsed = Arguments::parse(list, self.source);
        let origin = Origin::Call {
            line: one_based(call.start_position().row),
            column: one_based(call.start_position().column),
        };
        for statement in self.statements(&method, &parsed, call) {
            self.script.push(&statement, origin);
        }
    }

    fn statements(&self, method: &str, parsed: &Arguments<'tree>, call: Node<'tree>) -> Vec<String> {
        let Some(table) = parsed
            .positional
            .first()
            .and_then(|node| name(*node, self.source))
        else {
            return Vec::new();
        };
        let shifted = parsed.without_first();
        let block = call.child_by_field_name("block");
        match method {
            CREATE_TABLE => tables::create_table(&table, &shifted, block, self.source),
            CHANGE_TABLE => tables::change_table(&table, block, self.source),
            _ => statements::statement(method, &table, &shifted, self.source).unwrap_or_default(),
        }
    }

    fn execute(&mut self, list: Option<Node<'tree>>) {
        let Some(argument) = list.and_then(|list| named_children(list).into_iter().next()) else {
            return;
        };
        let (sql, start) = if let Some(body) = heredoc_body(argument) {
            (heredoc_text(body, self.source), body)
        } else if argument.kind() == STRING {
            let Some(sql) = string_content(argument, self.source) else {
                return;
            };
            (
                sql,
                named_children(argument).into_iter().next().unwrap_or(argument),
            )
        } else {
            return;
        };
        self.script.push(
            sql.trim_end().trim_end_matches(';'),
            Origin::Execute {
                first_line: one_based(start.start_position().row),
                script_line: 0,
            },
        );
    }
}

#[cfg(test)]
mod tests;
