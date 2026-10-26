use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, ATTRIBUTE, CALL, CLASS, COMMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LAMBDA, LEFT_FIELD,
    RIGHT_FIELD, SUBSCRIPT, TUPLE, VALUE_FIELD, named, text, unparenthesized,
};
use super::fix_edits::{commented_unsafe, pad};
use super::known_types::{Known, is_binding, is_known};
use super::loop_variable_reads::is_load;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::rules::PythonRule;

const FOR: &str = "for_statement";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const YIELD: &str = "yield";
const BODY_FIELD: &str = "body";
const ALTERNATIVE_FIELD: &str = "alternative";
const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const OBJECT_FIELD: &str = "object";
const ASYNC: &str = "async";
const ITEMS: &str = "items";
const ENUMERATE: &str = "enumerate";
const LIST: &str = "list";
const SEQUENCE_LITERALS: [&str; 3] = ["list", TUPLE, "set"];
const PAIR_TARGETS: [&str; 3] = ["pattern_list", "tuple_pattern", TUPLE];
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];
const YIELDED_TUPLES: [&str; 2] = [TUPLE, "expression_list"];
const TARGET_CONTAINERS: [&str; 5] = [
    "pattern_list",
    "tuple_pattern",
    TUPLE,
    "list",
    "parenthesized_expression",
];
const BLOCK: &str = "block";
const EXPRESSION_LIST: &str = "expression_list";
const DUMMY_PREFIX: char = '_';

// Chequeos sobre loops for: UP028, PLR1733, PLR1736 y PERF101.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == FOR)
        .flat_map(|statement| loop_reports(*statement, source))
        .collect()
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn loop_reports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut found: Vec<Report> = yield_in_for_loop(statement, source).into_iter().collect();
    found.extend(unnecessary_index_lookups(statement, source));
    found.extend(unnecessary_list_cast(statement, source));
    found
}
// UP028: `for x in y: yield x`, que es `yield from y`. Como ruff, si los nombres del loop no se leen
// en ningún otro lado del alcance.
fn yield_in_for_loop(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if is_async(statement, source) || statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let target = statement.child_by_field_name(LEFT_FIELD)?;
    let only = single_statement(statement.child_by_field_name(BODY_FIELD))
        .filter(|only| only.kind() == EXPRESSION_STATEMENT)?;
    let yielded = significant(only)
        .first()
        .copied()
        .filter(|value| value.kind() == YIELD)?;
    let is_from = text(yielded, source)
        .split_whitespace()
        .nth(1)
        .is_some_and(|word| word == "from");
    let value = *significant(yielded).first()?;
    let names = same_names(target, value, source)?;
    let scope = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()))?;
    if is_async(scope, source) {
        return None;
    }
    let mut pending = named(scope);
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            let yielded_name = node.start_byte() >= value.start_byte() && node.end_byte() <= value.end_byte();
            if !yielded_name && names.contains(&text(node, source)) && is_load(node) {
                return None;
            }
            continue;
        }
        if !SCOPES.contains(&node.kind()) {
            pending.extend(named(node));
        }
    }
    (!is_from).then(|| {
        let fix = statement.child_by_field_name(RIGHT_FIELD).map(|iterable| {
            let written = text(iterable, source);
            let replacement = if iterable.kind() == EXPRESSION_LIST {
                format!("yield from ({written})")
            } else {
                format!("yield from {written}")
            };
            Fix::replace(statement, &replacement, source).marked_unsafe()
        });
        Report::at(PythonRule::UP028, statement, "").with_fix(fix)
    })
}
// `is_same_expr` de ruff: el mismo nombre, o tuplas con los mismos nombres en el mismo orden.
fn same_names<'source>(
    target: Node<'_>,
    value: Node<'_>,
    source: &'source [u8],
) -> Option<Vec<&'source str>> {
    if target.kind() == IDENTIFIER {
        return (value.kind() == IDENTIFIER && text(value, source) == text(target, source))
            .then(|| vec![text(target, source)]);
    }
    if !PAIR_TARGETS.contains(&target.kind()) || !YIELDED_TUPLES.contains(&value.kind()) {
        return None;
    }
    let targets = significant(target);
    let values = significant(value);
    if targets.len() != values.len() {
        return None;
    }
    let mut names = Vec::new();
    for (inner_target, inner_value) in targets.iter().zip(&values) {
        names.extend(same_names(*inner_target, *inner_value, source)?);
    }
    Some(names)
}
// PLR1733 (`d[k]` en `for k, v in d.items()`) y PLR1736 (`s[i]` en `for i, x in enumerate(s)`):
// el valor ya está en la otra variable. Avisa en cada lectura, hasta que se reasigna algo.
fn unnecessary_index_lookups(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(target) = statement
        .child_by_field_name(LEFT_FIELD)
        .filter(|target| PAIR_TARGETS.contains(&target.kind()))
    else {
        return Vec::new();
    };
    let names = significant(target);
    let [index, value] = names.as_slice() else {
        return Vec::new();
    };
    if index.kind() != IDENTIFIER
        || value.kind() != IDENTIFIER
        || text(*value, source).starts_with(DUMMY_PREFIX)
    {
        return Vec::new();
    }
    let Some((rule, sequence)) = statement
        .child_by_field_name(RIGHT_FIELD)
        .and_then(|iterable| indexed_sequence(unparenthesized(iterable), source))
    else {
        return Vec::new();
    };
    let watched = [text(*index, source), text(*value, source), text(sequence, source)];
    let mut nodes = Vec::new();
    collect_in_order(statement.child_by_field_name(BODY_FIELD), &mut nodes);
    let stop = nodes
        .iter()
        .filter(|node| {
            (node.kind() == IDENTIFIER && watched.contains(&text(**node, source)) && is_binding(**node))
                || (node.kind() == SUBSCRIPT
                    && !is_load_subscript(**node)
                    && node
                        .child_by_field_name(VALUE_FIELD)
                        .is_some_and(|object| text(object, source) == text(sequence, source)))
        })
        .map(|node| enclosing_statement(*node).start_byte())
        .min()
        .unwrap_or(usize::MAX);
    let mut found = Vec::new();
    for node in nodes
        .into_iter()
        .filter(|node| node.start_byte() < stop && !is_in_target(*node))
    {
        let looks_up = node.kind() == SUBSCRIPT
            && node.child_by_field_name(VALUE_FIELD).is_some_and(|object| {
                object.kind() == IDENTIFIER && text(object, source) == text(sequence, source)
            })
            && matches!(significant(node).as_slice(), [_, key] if key.kind() == IDENTIFIER && text(*key, source) == text(*index, source))
            && is_load_subscript(node);
        if looks_up {
            let fix = lookup_fix(node, *index, *value, source);
            found.push(Report::at(rule, node, "").with_fix(Some(fix)));
        }
    }
    found
}
// Como ruff: el subíndice pasa a la variable del loop, con un no-op sobre las dos variables para que
// otro arreglo que las cambie no vaya en la misma vuelta.
fn lookup_fix(subscript: Node<'_>, index: Node<'_>, value: Node<'_>, source: &[u8]) -> Fix {
    let rewrite = |node: Node<'_>, replacement: &str| TextEdit {
        start: TextPoint::from_point(node.start_position()),
        end: TextPoint::from_point(node.end_position()),
        replacement: replacement.to_owned(),
    };
    let name = text(value, source);
    let edits = vec![
        rewrite(subscript, name),
        rewrite(index, text(index, source)),
        rewrite(value, name),
    ];
    Fix::from_edits(source, edits)
}

fn indexed_sequence<'tree>(iterable: Node<'tree>, source: &[u8]) -> Option<(PythonRule, Node<'tree>)> {
    if iterable.kind() != CALL {
        return None;
    }
    let function = iterable.child_by_field_name(FUNCTION_FIELD)?;
    let arguments = significant(iterable.child_by_field_name(ARGUMENTS_FIELD)?);
    if function.kind() == ATTRIBUTE {
        let object = function
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| object.kind() == IDENTIFIER)?;
        let is_items = function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == ITEMS);
        return (is_items && arguments.is_empty()).then_some((PythonRule::PLR1733, object));
    }
    match arguments.as_slice() {
        [sequence] if text(function, source) == ENUMERATE && sequence.kind() == IDENTIFIER => {
            Some((PythonRule::PLR1736, *sequence))
        }
        _ => None,
    }
}
fn collect_in_order<'tree>(node: Option<Node<'tree>>, nodes: &mut Vec<Node<'tree>>) {
    let Some(node) = node else {
        return;
    };
    nodes.push(node);
    for child in named(node) {
        collect_in_order(Some(child), nodes);
    }
}
fn is_load_subscript(subscript: Node<'_>) -> bool {
    let mut child = subscript;
    let mut parent = subscript.parent();
    while let Some(container) = parent.filter(|container| TARGET_CONTAINERS.contains(&container.kind())) {
        child = container;
        parent = container.parent();
    }
    parent.is_none_or(|parent| match parent.kind() {
        ASSIGNMENT | "augmented_assignment" | FOR => parent
            .child_by_field_name(LEFT_FIELD)
            .is_none_or(|left| left.id() != child.id()),
        "delete_statement" => false,
        _ => true,
    })
}
fn is_in_target(node: Node<'_>) -> bool {
    let mut child = node;
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if matches!(ancestor.kind(), ASSIGNMENT | "augmented_assignment")
            && ancestor
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.id() == child.id())
        {
            return true;
        }
        if ancestor.kind() == BLOCK {
            return false;
        }
        child = ancestor;
    }
    false
}
fn enclosing_statement(node: Node<'_>) -> Node<'_> {
    std::iter::successors(Some(node), Node::parent)
        .find(|ancestor| ancestor.parent().is_some_and(|parent| parent.kind() == BLOCK))
        .unwrap_or(node)
}
// PERF101: `for x in list(y)` cuando `y` ya se puede recorrer y el loop no lo toca.
fn unnecessary_list_cast(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let call = unparenthesized(statement.child_by_field_name(RIGHT_FIELD)?);
    let is_list = call.kind() == CALL
        && call
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| text(function, source) == LIST);
    if !is_list {
        return None;
    }
    let arguments = significant(call.child_by_field_name(ARGUMENTS_FIELD)?);
    let [iterable] = arguments.as_slice() else {
        return None;
    };
    let iterable = unparenthesized(*iterable);
    let iterable_ok = SEQUENCE_LITERALS.contains(&iterable.kind())
        || (iterable.kind() == IDENTIFIER
            && [Known::List, Known::Tuple, Known::Set]
                .into_iter()
                .any(|kind| is_known(iterable, kind, source))
            && !mentions(
                statement.child_by_field_name(BODY_FIELD),
                text(iterable, source),
                source,
            ));
    iterable_ok.then(|| {
        let replacement = pad(text(iterable, source).to_owned(), call, source);
        let fix = commented_unsafe(Fix::replace(call, &replacement, source), call, source);
        Report::at(PythonRule::PERF101, call, "").with_fix(Some(fix))
    })
}
fn mentions(body: Option<Node<'_>>, wanted: &str, source: &[u8]) -> bool {
    let mut nodes = Vec::new();
    collect_in_order(body, &mut nodes);
    nodes
        .into_iter()
        .any(|node| node.kind() == IDENTIFIER && text(node, source) == wanted)
}
fn single_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let statements = significant(body?);
    match statements.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}
fn is_async(statement: Node<'_>, source: &[u8]) -> bool {
    statement
        .child(0)
        .is_some_and(|first| text(first, source) == ASYNC)
}
