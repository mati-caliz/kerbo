mod calls;
mod dialects;
mod operations;
mod render;
mod script;
mod tables;

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::Violation;
use crate::syntax::{named_children, one_based};
use calls::{Call, string_value};
use operations::{Context, Target, string_or_binding};
use render::Constants;
use script::{Origin, Script};

const ALEMBIC_MARKERS: &[&str] = &["from alembic import", "import alembic"];
const UPGRADE: &str = "upgrade";
const OPERATIONS: &str = "op";
const BATCH: &str = "batch_alter_table";
const AUTOCOMMIT: &str = "autocommit_block";
const EXECUTE: &str = "execute";
const TEXT_FUNCTION: &str = "text";
const DOWN_REVISION: &str = "down_revision";
const NONE: &str = "none";
const FUNCTION_DEFINITION: &str = "function_definition";
const WITH_CLAUSE: &str = "with_clause";
const ASSIGNMENT: &str = "assignment";
const LITERAL_SEQUENCES: &[&str] = &["list", "tuple"];

struct Walker<'tree> {
    source: &'tree [u8],
    helpers: HashMap<&'tree str, Node<'tree>>,
    constants: Constants<'tree>,
    active_helpers: HashSet<&'tree str>,
    batches: HashMap<String, String>,
    bindings: Vec<HashMap<String, String>>,
    script: Script,
}

pub fn violations(root: Node<'_>, source: &[u8]) -> Vec<Violation> {
    let text = String::from_utf8_lossy(source);
    if !ALEMBIC_MARKERS.iter().any(|marker| text.contains(marker)) {
        return Vec::new();
    }
    let functions: HashMap<&str, Node<'_>> = named_children(root)
        .into_iter()
        .filter(|node| node.kind() == FUNCTION_DEFINITION)
        .filter_map(|node| Some((node.child_by_field_name("name")?.utf8_text(source).ok()?, node)))
        .collect();
    let Some(upgrade) = functions.get(UPGRADE).copied() else {
        return Vec::new();
    };
    let mut walker = Walker {
        source,
        helpers: functions,
        constants: module_constants(root, source),
        active_helpers: HashSet::from([UPGRADE]),
        batches: HashMap::new(),
        bindings: Vec::new(),
        script: Script::new(),
    };
    if let Some(body) = upgrade.child_by_field_name("body") {
        walker.walk(body);
    }
    walker.script.violations(is_first_revision(root, source))
}

fn module_constants<'tree>(root: Node<'tree>, source: &'tree [u8]) -> Constants<'tree> {
    named_children(root)
        .into_iter()
        .filter_map(|statement| named_children(statement).into_iter().next())
        .filter(|assignment| assignment.kind() == ASSIGNMENT)
        .filter_map(|assignment| {
            let left = assignment
                .child_by_field_name("left")
                .filter(|left| left.kind() == "identifier")?;
            Some((
                left.utf8_text(source).ok()?,
                assignment.child_by_field_name("right")?,
            ))
        })
        .collect()
}

fn is_first_revision(root: Node<'_>, source: &[u8]) -> bool {
    named_children(root)
        .into_iter()
        .filter_map(|statement| named_children(statement).into_iter().next())
        .filter(|assignment| assignment.kind() == ASSIGNMENT)
        .find(|assignment| {
            assignment
                .child_by_field_name("left")
                .and_then(|left| left.utf8_text(source).ok())
                == Some(DOWN_REVISION)
        })
        .and_then(|assignment| assignment.child_by_field_name("right"))
        .is_some_and(|right| right.kind() == NONE)
}

impl<'tree> Walker<'tree> {
    fn lookup(&self, name: &str) -> Option<String> {
        self.bindings
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn value(&self, node: Node<'_>) -> Option<String> {
        string_or_binding(node, self.source, &|name| {
            self.lookup(name).or_else(|| {
                self.constants
                    .get(name)
                    .and_then(|constant| string_value(*constant, self.source))
            })
        })
    }

    fn walk(&mut self, node: Node<'tree>) {
        match node.kind() {
            "with_statement" => self.with_statement(node),
            "try_statement" => {
                if let Some(body) = node.child_by_field_name("body") {
                    self.walk(body);
                }
            }
            "assignment" => self.remember_assignment(node),
            "for_statement" if self.unrolled(node) => {}
            "call" => self.call(node),
            _ => self.walk_children(node),
        }
    }

    fn walk_children(&mut self, node: Node<'tree>) {
        for child in dialects::postgres_children(node, self.source) {
            self.walk(child);
            if dialects::returns_for_postgres(child, self.source) {
                return;
            }
        }
    }

    fn with_statement(&mut self, node: Node<'tree>) {
        let items: Vec<Node<'tree>> = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == WITH_CLAUSE)
            .flat_map(named_children)
            .collect();
        let mut autocommit = false;
        for item in &items {
            let value = item.child_by_field_name("value");
            let target = value.filter(|value| value.kind() == "as_pattern");
            let called = target
                .and_then(|pattern| {
                    named_children(pattern)
                        .into_iter()
                        .find_map(|child| Call::parse(child, self.source))
                })
                .or_else(|| value.and_then(|value| Call::parse(value, self.source)));
            match called {
                Some(call) if call.method == AUTOCOMMIT => autocommit = true,
                Some(call) if call.receiver == OPERATIONS && call.method == BATCH => {
                    self.remember_batch(&call, target);
                }
                Some(_) | None => {}
            }
        }
        if autocommit {
            self.script.leave_transaction();
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.walk(body);
        }
        if autocommit {
            self.script.enter_transaction();
        }
    }

    fn remember_batch(&mut self, call: &Call<'_>, pattern: Option<Node<'_>>) {
        let alias = pattern
            .and_then(|pattern| pattern.child_by_field_name("alias"))
            .and_then(|alias| alias.utf8_text(self.source).ok());
        if let Some(alias) = alias {
            let table = call
                .positional(0)
                .and_then(|node| self.value(node))
                .unwrap_or_default();
            self.batches.insert(alias.to_owned(), table);
        }
    }

    fn unrolled(&mut self, node: Node<'tree>) -> bool {
        let variable = node
            .child_by_field_name("left")
            .filter(|left| left.kind() == "identifier")
            .and_then(|left| left.utf8_text(self.source).ok());
        let items = node
            .child_by_field_name("right")
            .map(|right| self.constant_or_itself(right))
            .filter(|right| LITERAL_SEQUENCES.contains(&right.kind()))
            .map(named_children);
        let (Some(variable), Some(items), Some(body)) = (variable, items, node.child_by_field_name("body"))
        else {
            return false;
        };
        let values: Option<Vec<String>> = items.iter().map(|item| self.value(*item)).collect();
        let Some(values) = values else {
            return false;
        };
        for value in values {
            self.bindings.push(HashMap::from([(variable.to_owned(), value)]));
            self.walk(body);
            self.bindings.pop();
        }
        true
    }

    fn remember_assignment(&mut self, node: Node<'tree>) {
        let left = node
            .child_by_field_name("left")
            .filter(|left| left.kind() == "identifier")
            .and_then(|left| left.utf8_text(self.source).ok());
        if let (Some(name), Some(right)) = (left, node.child_by_field_name("right")) {
            self.constants.insert(name, right);
        }
        self.walk_children(node);
    }

    fn constant_or_itself(&self, node: Node<'tree>) -> Node<'tree> {
        if node.kind() != "identifier" {
            return node;
        }
        node.utf8_text(self.source)
            .ok()
            .and_then(|name| self.constants.get(name))
            .copied()
            .unwrap_or(node)
    }

    fn call(&mut self, node: Node<'tree>) {
        let Some(call) = Call::parse(node, self.source) else {
            return;
        };
        if call.method == EXECUTE {
            self.execute(&call);
            return;
        }
        let batch_table = self.batches.get(call.receiver).cloned();
        if call.receiver == OPERATIONS || batch_table.is_some() {
            let target = Target {
                batch_table: batch_table.as_deref(),
            };
            let origin = Origin::Call {
                line: call.line,
                column: call.column,
            };
            let value = |argument: Node<'_>| self.value(argument);
            let context = Context {
                value: &value,
                constants: &self.constants,
            };
            for statement in operations::statements(&call, target, &context) {
                self.script.push(&statement, origin);
            }
            return;
        }
        if call.receiver.is_empty() && self.inline_helper(&call) {
            return;
        }
        for child in named_children(node) {
            self.walk(child);
        }
    }

    fn inline_helper(&mut self, call: &Call<'tree>) -> bool {
        let Some(helper) = self.helpers.get(call.method).copied() else {
            return false;
        };
        if !self.active_helpers.insert(call.method) {
            return false;
        }
        let parameters = helper
            .child_by_field_name("parameters")
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|parameter| parameter.utf8_text(self.source).ok())
            .map(|parameter| {
                parameter
                    .split([':', '='])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            });
        let bound: HashMap<String, String> = parameters
            .zip(call.all_positional())
            .filter_map(|(parameter, argument)| Some((parameter, self.value(argument)?)))
            .collect();
        self.bindings.push(bound);
        if let Some(body) = helper.child_by_field_name("body") {
            self.walk(body);
        }
        self.bindings.pop();
        self.active_helpers.remove(call.method);
        true
    }

    fn execute(&mut self, call: &Call<'_>) {
        let Some(statement) = call.positional(0) else {
            return;
        };
        let literal = Call::parse(statement, self.source)
            .filter(|inner| inner.method == TEXT_FUNCTION)
            .and_then(|inner| inner.positional(0))
            .unwrap_or(statement);
        if let Some(sql) = self.value(literal) {
            let first_line = one_based(literal.start_position().row);
            self.script.push(
                &sql,
                Origin::Execute {
                    first_line,
                    script_line: 0,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests;
