use std::collections::HashMap;

use tree_sitter::Node;

use super::Report;
use super::ast::{BODY_FIELD, CLASS, FUNCTION, IDENTIFIER, LAMBDA, MODULE, NAME_FIELD, named, text};
use super::known_types::is_name_reference;
use super::methods::{MethodKind, method_kind};
use crate::rules::PythonRule;

const SCOPES: [&str; 8] = [
    FUNCTION,
    CLASS,
    LAMBDA,
    MODULE,
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];
const YIELD: &str = "yield";
const FROM: &str = "from";
const ASYNC: &str = "async";
const INIT: &str = "__init__";
const GLOBAL: &str = "global_statement";
const NONLOCAL: &str = "nonlocal_statement";
const NOT_EXPRESSIONS: [&str; 4] = [GLOBAL, NONLOCAL, "aliased_import", "dotted_name"];
const AS_PATTERN_TARGET: &str = "as_pattern_target";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];

// Un nombre declarado `global` en un alcance: dónde está la declaración.
struct Declaration<'tree> {
    name: Node<'tree>,
    statement: Node<'tree>,
}

// Reglas de pylint que dependen del alcance: PLE0100 (`yield` en `__init__`), PLE1700 (`yield from`
// en una función `async`), PLW0604 (`global` en el módulo), PLE0118 (un nombre usado antes de su
// `global`) y PLE0115 (un nombre `nonlocal` y `global` a la vez).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    let mut globals: HashMap<usize, (Node<'_>, HashMap<String, Declaration<'_>>)> = HashMap::new();
    for node in nodes.iter().filter(|node| node.is_named()) {
        match node.kind() {
            YIELD => reports.extend(yield_reports(*node, source)),
            GLOBAL => {
                let Some(scope) = enclosing_scope(*node) else {
                    continue;
                };
                if scope.kind() == MODULE {
                    reports.push(Report::at(PythonRule::PLW0604, *node, ""));
                }
                let (_, declared) = globals
                    .entry(scope.id())
                    .or_insert_with(|| (scope, HashMap::new()));
                for name in named(*node).into_iter().filter(|name| name.kind() == IDENTIFIER) {
                    declared
                        .entry(text(name, source).to_owned())
                        .or_insert(Declaration {
                            name,
                            statement: *node,
                        });
                }
            }
            _ => {}
        }
    }
    for (scope, declared) in globals.values() {
        reports.extend(loads_before_global(*scope, declared, source));
    }
    for nonlocal in nodes.iter().filter(|node| node.kind() == NONLOCAL) {
        let Some((_, declared)) = enclosing_scope(*nonlocal).and_then(|scope| globals.get(&scope.id()))
        else {
            continue;
        };
        let conflicting = named(*nonlocal)
            .into_iter()
            .filter(|name| name.kind() == IDENTIFIER)
            .filter_map(|name| declared.get(text(name, source)));
        reports.extend(conflicting.map(|declaration| Report::at(PythonRule::PLE0115, declaration.name, "")));
    }
    reports
}

fn enclosing_scope(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent).find(|ancestor| SCOPES.contains(&ancestor.kind()))
}

fn yield_reports(node: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(function) = enclosing_scope(node).filter(|scope| scope.kind() == FUNCTION) else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    let is_init = function
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == INIT);
    if is_init && method_kind(function, source) == Some(MethodKind::Instance) {
        reports.push(Report::at(PythonRule::PLE0100, node, ""));
    }
    let is_yield_from = node.child(1).is_some_and(|keyword| text(keyword, source) == FROM);
    let is_async = function
        .child(0)
        .is_some_and(|first| text(first, source) == ASYNC);
    if is_yield_from && is_async {
        reports.push(Report::at(PythonRule::PLE1700, node, ""));
    }
    reports
}

// PLE0118: en el alcance del `global`, cada nombre (leído, asignado o borrado) que aparece antes de la
// declaración. Avisa en el nombre.
fn loads_before_global(
    scope: Node<'_>,
    declared: &HashMap<String, Declaration<'_>>,
    source: &[u8],
) -> Vec<Report> {
    let body = if scope.kind() == MODULE {
        Some(scope)
    } else {
        scope.child_by_field_name(BODY_FIELD)
    };
    let mut pending: Vec<Node<'_>> = body.into_iter().collect();
    let mut reports = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            let early = declared
                .get(text(node, source))
                .is_some_and(|declaration| node.start_byte() < declaration.statement.start_byte());
            if early && is_name_expression(node) {
                reports.push(Report::at(PythonRule::PLE0118, node, text(node, source)));
            }
            continue;
        }
        if node.id() != scope.id() && SCOPES.contains(&node.kind()) {
            continue;
        }
        pending.extend(named(node));
    }
    reports
}

fn is_name_expression(identifier: Node<'_>) -> bool {
    let Some(parent) = identifier.parent() else {
        return true;
    };
    let exception_name = parent.kind() == AS_PATTERN_TARGET
        && std::iter::successors(parent.parent(), Node::parent)
            .nth(1)
            .is_some_and(|clause| EXCEPT_CLAUSES.contains(&clause.kind()));
    is_name_reference(identifier) && !NOT_EXPRESSIONS.contains(&parent.kind()) && !exception_name
}
