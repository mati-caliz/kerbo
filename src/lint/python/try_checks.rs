use std::collections::HashSet;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, BLOCK, CALL, COMMENT, EXPRESSION_STATEMENT, FUNCTION_FIELD, IDENTIFIER,
    OBJECT_FIELD, TUPLE, named, text, unparenthesized,
};
use super::codegen;
use super::fix_edits::{commented_unsafe, pad};
use crate::fix::Fix;
use crate::rules::PythonRule;

const TRY: &str = "try_statement";
const ASSERT: &str = "assert_statement";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const FINALLY: &str = "finally_clause";
const RETURN: &str = "return_statement";
const AS_PATTERN: &str = "as_pattern";
const UNCALLED_MOCK_METHODS: [&str; 14] = [
    "assert_any_call",
    "assert_called",
    "assert_called_once",
    "assert_called_once_with",
    "assert_called_with",
    "assert_has_calls",
    "assert_not_called",
    "assert_awaited",
    "assert_awaited_once",
    "assert_awaited_with",
    "assert_awaited_once_with",
    "assert_any_await",
    "assert_has_awaits",
    "assert_not_awaited",
];
const MISSING_MOCK_METHODS: [&str; 13] = [
    "any_call",
    "called_once",
    "called_once_with",
    "called_with",
    "has_calls",
    "not_called",
    "awaited",
    "awaited_once",
    "awaited_with",
    "awaited_once_with",
    "any_await",
    "has_awaits",
    "not_awaited",
];

// Reglas de un `try`: B014 (excepción repetida en la tupla de un `except`), B025 (excepción que ya
// atrapó un `except` anterior), B029 (`except ():`, que no atrapa nada) y SIM107 (`return` en el
// `finally` que pisa el del `try` o el `except`). Y PGH005: métodos de un mock sin llamar o que no
// existen.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    for node in nodes.iter().filter(|node| node.is_named()) {
        match node.kind() {
            TRY => reports.extend(try_reports(*node, source)),
            EXPRESSION_STATEMENT => reports.extend(uncalled_mock_method(*node, source)),
            ASSERT => reports.extend(missing_mock_method(*node, source)),
            _ => {}
        }
    }
    reports
}

fn try_reports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let clauses = named(statement);
    let handlers: Vec<Node<'_>> = clauses
        .iter()
        .filter(|clause| EXCEPT_CLAUSES.contains(&clause.kind()))
        .copied()
        .collect();
    let mut reports = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for handler in &handlers {
        let Some(caught) = caught(*handler) else {
            continue;
        };
        if caught.kind() == TUPLE {
            reports.extend(tuple_reports(*handler, caught, source, &mut seen));
        } else if let Some(name) = dotted_name(caught, source)
            && !seen.insert(name)
        {
            reports.push(Report::at(PythonRule::B025, caught, ""));
        }
    }
    reports.extend(return_in_finally(&clauses, &handlers));
    reports
}

// B029, B014 y B025 para un `except` con una tupla: vacía, con un nombre repetido, o con uno que ya
// atrapó un `except` anterior.
fn tuple_reports(
    handler: Node<'_>,
    caught: Node<'_>,
    source: &[u8],
    seen: &mut HashSet<String>,
) -> Vec<Report> {
    let elements = named(caught);
    let mut reports = Vec::new();
    if elements.is_empty() {
        reports.push(Report::at(PythonRule::B029, handler, ""));
    }
    let mut in_tuple: HashSet<String> = HashSet::new();
    let mut repeated = false;
    let mut unique: Vec<Node<'_>> = Vec::new();
    for (element, name) in elements
        .iter()
        .copied()
        .filter_map(|element| dotted_name(element, source).map(|name| (element, name)))
    {
        if !in_tuple.insert(name.clone()) {
            repeated = true;
            continue;
        }
        unique.push(element);
        if !seen.insert(name) {
            reports.push(Report::at(PythonRule::B025, element, ""));
        }
    }
    if repeated {
        let unnamed = elements
            .iter()
            .filter(|element| dotted_name(**element, source).is_none());
        unique.extend(unnamed);
        let replacement = if let [only] = unique.as_slice() {
            pad(codegen::unparse(*only, 0, source), caught, source)
        } else {
            let listed: Vec<String> = unique
                .iter()
                .map(|element| codegen::unparse(*element, codegen::COMMA, source))
                .collect();
            format!("({})", listed.join(", "))
        };
        let fix = commented_unsafe(Fix::replace(caught, &replacement, source), caught, source);
        reports.push(Report::at(PythonRule::B014, caught, "").with_fix(Some(fix)));
    }
    reports
}

// Lo que atrapa un `except`, sin el `as`.
fn caught(handler: Node<'_>) -> Option<Node<'_>> {
    let first = named(handler)
        .into_iter()
        .find(|child| !matches!(child.kind(), COMMENT | BLOCK))?;
    let first = if first.kind() == AS_PATTERN {
        named(first).into_iter().next()?
    } else {
        first
    };
    Some(unparenthesized(first))
}

// `UnqualifiedName`: `x` o `a.b.c`, sólo nombres.
fn dotted_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let node = unparenthesized(node);
    match node.kind() {
        IDENTIFIER => Some(text(node, source).to_owned()),
        ATTRIBUTE => {
            let object = dotted_name(node.child_by_field_name(OBJECT_FIELD)?, source)?;
            let attribute = text(node.child_by_field_name(ATTRIBUTE_FIELD)?, source);
            Some(format!("{object}.{attribute}"))
        }
        _ => None,
    }
}

fn block_statements(clause: Node<'_>) -> Vec<Node<'_>> {
    named(clause)
        .into_iter()
        .find(|child| child.kind() == BLOCK)
        .map(named)
        .unwrap_or_default()
}

fn return_in_finally(clauses: &[Node<'_>], handlers: &[Node<'_>]) -> Option<Report> {
    let finally = clauses.iter().find(|clause| clause.kind() == FINALLY)?;
    let body = clauses.iter().find(|clause| clause.kind() == BLOCK)?;
    let returns =
        |statements: Vec<Node<'_>>| statements.into_iter().any(|statement| statement.kind() == RETURN);
    let earlier = returns(named(*body)) || handlers.iter().any(|handler| returns(block_statements(*handler)));
    if !earlier {
        return None;
    }
    block_statements(*finally)
        .into_iter()
        .find(|statement| statement.kind() == RETURN)
        .map(|statement| Report::at(PythonRule::SIM107, statement, ""))
}

fn uncalled_mock_method(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let children = named(statement);
    let [value] = children.as_slice() else {
        return None;
    };
    (value.kind() == ATTRIBUTE
        && value
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| UNCALLED_MOCK_METHODS.contains(&text(attribute, source))))
    .then(|| Report::at(PythonRule::PGH005, *value, ""))
}

fn missing_mock_method(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let test = unparenthesized(*named(statement).first()?);
    let attribute = match test.kind() {
        ATTRIBUTE => Some(test),
        CALL => test
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE),
        _ => None,
    }?;
    attribute
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|name| MISSING_MOCK_METHODS.contains(&text(name, source)))
        .then(|| Report::at(PythonRule::PGH005, test, ""))
}
