mod arguments;
mod columns;
mod ddl;
mod guards;
mod history;
mod locals;
mod loops;
mod migration;
mod references;
mod rewrite;
mod schema;
mod schema_table;
mod sha256;
mod sql_tokens;
mod statements;
mod tables;

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use tree_sitter::Node;

use super::Violation;
use super::script::{Origin, Script};
use crate::syntax::{named_children, one_based};
use arguments::{Arguments, heredoc_body, heredoc_text, name};
use migration::Compatibility;
use schema::Schema;

pub use history::fingerprint;

pub const EXTENSION: &str = "rb";

const DATABASE_DIRECTORY: &str = "db";
const MIGRATE_DIRECTORY: &str = "migrate";
const CALL: &str = "call";
const IDENTIFIER: &str = "identifier";
const STRING: &str = "string";
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
const DOWN_DIRECTION: &str = "down";
const EXECUTE: &str = "execute";
const CREATE_TABLE: &str = "create_table";
const CHANGE_TABLE: &str = "change_table";
const IF_EXISTS: &str = "if_exists";
const REMOVE_INDEX: &str = "remove_index";
const INDEX_NAME: &str = "name";
const DROPS_IF_EXISTS: &[&str] = &[REMOVE_INDEX, "remove_foreign_key"];
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
    if let Some(expansion) = rewrite::rewritten(root, source)
        && let Some(tree) = rewrite::parse(&expansion.source)
    {
        return evaluate_tree(
            tree.root_node(),
            expansion.source.as_bytes(),
            schema,
            expansion.rows,
        );
    }
    evaluate_tree(root, source, schema, Vec::new())
}

fn evaluate_tree(
    root: Node<'_>,
    source: &[u8],
    schema: Schema,
    rows: Vec<usize>,
) -> Option<(Script, Schema)> {
    let class = migration::migration_class(root, source)?;
    let transactional = !migration::disables_transaction(class, source);
    let mut walker = Walker {
        source,
        rows,
        compatibility: migration::compatibility(class, source),
        returned: false,
        uncertain: 0,
        helpers: migration::helpers(root, class, source),
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

struct Walker<'tree, 'source> {
    source: &'source [u8],
    rows: Vec<usize>,
    compatibility: Compatibility,
    returned: bool,
    uncertain: usize,
    helpers: HashMap<String, Node<'tree>>,
    visiting: HashSet<String>,
    schema: Schema,
    script: Script,
}

impl<'tree> Walker<'tree, '_> {
    fn original_line(&self, row: usize) -> usize {
        one_based(self.rows.get(row).copied().unwrap_or(row))
    }

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
            } else if method == EXECUTE {
                self.execute(call.child_by_field_name("arguments"));
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
        let parsed = Arguments::parse(list, self.source).with_compatibility(self.compatibility);
        let origin = Origin::Call {
            line: self.original_line(call.start_position().row),
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
            CHANGE_TABLE => tables::change_table(&table, block, self.compatibility, self.source),
            CREATE_JOIN_TABLE => tables::create_join_table(&table, &shifted, block, self.source),
            REMOVE_INDEX => self.remove_index(&table, &shifted),
            _ => statements::statement(method, &table, &shifted, self.source).unwrap_or_default(),
        }
    }

    fn remove_index(&self, table: &str, arguments: &Arguments<'tree>) -> Vec<String> {
        let existing = (!arguments.has(INDEX_NAME))
            .then(|| ddl::removed_index_columns(arguments, self.source))
            .filter(|columns| !columns.is_empty())
            .and_then(|columns| self.schema.index_name_on(table, &columns));
        vec![existing.map_or_else(
            || ddl::drop_index(table, arguments, self.source),
            |name| ddl::drop_named_index(&name, arguments, self.source),
        )]
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
            let Some(sql) = arguments::executed_sql(argument, self.source) else {
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
                first_line: self.original_line(start.start_position().row),
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
mod dump_tests;
#[cfg(test)]
mod loops_tests;
#[cfg(test)]
mod rewrite_tests;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod tests;
