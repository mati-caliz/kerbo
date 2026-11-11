mod arguments;
mod columns;
mod ddl;
mod guards;
mod history;
mod references;
mod schema;
mod sha256;
mod sql_tokens;
mod statements;
mod tables;

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use tree_sitter::Node;

use super::Violation;
use super::script::{Origin, Script};
use crate::syntax::{descendants, named_children, one_based};
use arguments::{Arguments, heredoc_body, heredoc_text, name, string_content};
use schema::Schema;

pub use history::fingerprint;

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
const RECEIVER_BLOCKS: &[&str] = &[UP_DIRECTION, "transaction"];
const RETURN: &str = "return";
const SQUISH: &str = "squish";
const HEREDOC_FILTERS: &[&str] = &[SQUISH, "strip_heredoc"];
const CREATE_JOIN_TABLE: &str = "create_join_table";
const VERSION_OPEN: char = '[';
const VERSION_CLOSE: char = ']';
const VERSION_SEPARATOR: char = '.';
const FIRST_BIGINT_VERSION: (u32, u32) = (5, 1);
const DOWN_DIRECTION: &str = "down";
const EXECUTE: &str = "execute";
const CREATE_TABLE: &str = "create_table";
const CHANGE_TABLE: &str = "change_table";
const IF_EXISTS: &str = "if_exists";
const DROPS_IF_EXISTS: &[&str] = &["remove_index", "remove_foreign_key"];
const SKIPPED_KINDS: &[&str] = &[
    "string",
    "heredoc_body",
    "lambda",
    "comment",
    "rescue",
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

pub fn violations(root: Node<'_>, source: &[u8], path: Option<&Path>) -> Vec<Violation> {
    let schema = path.map(history::schema_before).unwrap_or_default();
    evaluate(root, source, schema)
        .map(|(script, _)| script.finish())
        .unwrap_or_default()
}

fn evaluate(root: Node<'_>, source: &[u8], schema: Schema) -> Option<(Script, Schema)> {
    let class = migration_class(root, source)?;
    let transactional = !disables_transaction(class, source);
    let mut walker = Walker {
        source,
        integer_ids: uses_integer_ids(class, source),
        returned: false,
        uncertain: 0,
        helpers: helpers(class, source),
        visiting: HashSet::new(),
        schema,
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
    Some((script, walker.schema))
}

fn migration_class<'tree>(root: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    descendants(root).into_iter().find(|node| {
        node.kind() == CLASS
            && node
                .child_by_field_name("superclass")
                .is_some_and(|superclass| arguments::raw(superclass, source).contains(MIGRATION_MARKER))
    })
}

fn uses_integer_ids(class: Node<'_>, source: &[u8]) -> bool {
    let superclass = class
        .child_by_field_name("superclass")
        .map(|superclass| arguments::raw(superclass, source))
        .unwrap_or_default();
    let Some(version) = superclass
        .split_once(VERSION_OPEN)
        .and_then(|(_, rest)| rest.split_once(VERSION_CLOSE))
        .map(|(version, _)| version)
    else {
        return true;
    };
    let mut parts = version
        .split(VERSION_SEPARATOR)
        .map(|part| part.trim().parse::<u32>().unwrap_or_default());
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    (major, minor) < FIRST_BIGINT_VERSION
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
    integer_ids: bool,
    returned: bool,
    uncertain: usize,
    helpers: HashMap<String, Node<'tree>>,
    visiting: HashSet<String>,
    schema: Schema,
    script: Script,
}

impl<'tree> Walker<'tree, '_> {
    fn walk(&mut self, node: Node<'tree>) {
        if self.returned {
            return;
        }
        match node.kind() {
            CALL => self.call(node),
            IDENTIFIER => self.helper(&arguments::raw(node, self.source)),
            RETURN => self.returned = self.uncertain == 0,
            kind if guards::CONDITIONALS.contains(&kind) => self.conditional(node),
            kind if SKIPPED_KINDS.contains(&kind) => {}
            _ => named_children(node)
                .into_iter()
                .for_each(|child| self.walk(child)),
        }
    }

    fn conditional(&mut self, node: Node<'tree>) {
        if let Some(branches) = guards::branches(node, &self.schema, self.source) {
            for branch in branches {
                self.walk(branch);
            }
            return;
        }
        self.uncertain += 1;
        for child in named_children(node) {
            self.walk(child);
        }
        self.uncertain -= 1;
    }

    fn helper(&mut self, method: &str) {
        let Some(body) = self.helpers.get(method).copied() else {
            return;
        };
        if !self.visiting.insert(method.to_owned()) {
            return;
        }
        self.walk(body);
        self.returned = false;
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
            if RECEIVER_BLOCKS.contains(&method.as_str()) {
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
        let parsed = Arguments::parse(list, self.source).with_integer_ids(self.integer_ids);
        let origin = Origin::Call {
            line: one_based(call.start_position().row),
            column: one_based(call.start_position().column),
        };
        let table = parsed
            .positional
            .first()
            .and_then(|node| name(*node, self.source))
            .unwrap_or_default();
        let drops_if_exists = DROPS_IF_EXISTS.contains(&method.as_str()) && parsed.is_true(IF_EXISTS);
        for statement in self.statements(&method, &parsed, call) {
            if drops_if_exists && self.schema.drops_missing_object(&table, &statement) {
                continue;
            }
            let follow_ups = self.schema.apply(&statement);
            self.script.push(&statement, origin);
            for follow_up in follow_ups {
                self.script.push(&follow_up, origin);
            }
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
            CHANGE_TABLE => tables::change_table(&table, block, self.integer_ids, self.source),
            CREATE_JOIN_TABLE => tables::create_join_table(&table, &shifted, block, self.source),
            _ => statements::statement(method, &table, &shifted, self.source).unwrap_or_default(),
        }
    }

    fn execute(&mut self, list: Option<Node<'tree>>) {
        let Some(argument) = list.and_then(|list| named_children(list).into_iter().next()) else {
            return;
        };
        let (sql, start) = if let Some(body) = heredoc_body(argument) {
            (heredoc_text(body, self.source), body)
        } else if let Some((sql, body)) = self.filtered_heredoc(argument) {
            (sql, body)
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
        let sql = sql.trim_end().trim_end_matches(';');
        self.schema.apply(sql);
        self.script.push(
            sql,
            Origin::Execute {
                first_line: one_based(start.start_position().row),
                script_line: 0,
            },
        );
    }

    fn filtered_heredoc(&self, argument: Node<'tree>) -> Option<(String, Node<'tree>)> {
        if argument.kind() != CALL {
            return None;
        }
        let method = arguments::raw(argument.child_by_field_name("method")?, self.source);
        if !HEREDOC_FILTERS.contains(&method.as_str()) {
            return None;
        }
        let body = heredoc_body(argument.child_by_field_name("receiver")?)?;
        let sql = heredoc_text(body, self.source);
        if method == SQUISH {
            return Some((sql.split_whitespace().collect::<Vec<_>>().join(" "), body));
        }
        Some((sql, body))
    }
}

#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod tests;
