use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, ATTRIBUTE, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, SUBSCRIPT,
    TUPLE, VALUE_FIELD, named, text, unparenthesized,
};
use super::branches::equivalent;
use super::known_types::{Known, is_known};
use crate::rules::PythonRule;

const BODY_FIELD: &str = "body";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const OBJECT_FIELD: &str = "object";
const OPERATORS_FIELD: &str = "operators";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const PARENTHESIZED: &str = "parenthesized_expression";
const FOR: &str = "for_statement";
const IF: &str = "if_statement";
const DELETE: &str = "delete_statement";
const COMPARISON: &str = "comparison_operator";
const KEYS: &str = "keys";
const APPEND: &str = "append";
const IN: &str = "in";
const ASYNC: &str = "async";

// Loops y condiciones sobre diccionarios y listas: PLC0206, RUF051 y PERF402.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            FOR => found.extend(
                dict_index_missing_items(*node, source)
                    .into_iter()
                    .chain(manual_list_copy(*node, source)),
            ),
            IF => found.extend(key_in_dict_del(*node, source)),
            _ => {}
        }
    }
    found
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn is_load_subscript(subscript: Node<'_>) -> bool {
    let mut child = subscript;
    let mut parent = subscript.parent();
    while let Some(container) = parent.filter(|container| {
        matches!(
            container.kind(),
            "pattern_list" | "tuple_pattern" | TUPLE | "list" | PARENTHESIZED
        )
    }) {
        child = container;
        parent = container.parent();
    }
    parent.is_none_or(|parent| match parent.kind() {
        ASSIGNMENT | "augmented_assignment" | FOR => parent
            .child_by_field_name(LEFT_FIELD)
            .is_none_or(|left| left.id() != child.id()),
        DELETE => false,
        _ => true,
    })
}
// PLC0206: `for key in d:` que lee `d[key]` en vez de recorrer `d.items()`. Como ruff, no entra en
// un subíndice que se lee (`d[key][0]` no cuenta) pero sí en uno que se asigna. Avisa en la variable.
fn dict_index_missing_items(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let target = statement.child_by_field_name(LEFT_FIELD)?;
    let iterable = unparenthesized(statement.child_by_field_name(RIGHT_FIELD)?);
    let dictionary = if iterable.kind() == CALL {
        let function = iterable
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE)?;
        let no_arguments = iterable
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| significant(arguments).is_empty());
        if !no_arguments
            || function
                .child_by_field_name(ATTRIBUTE_FIELD)
                .is_none_or(|attribute| text(attribute, source) != KEYS)
        {
            return None;
        }
        function.child_by_field_name(OBJECT_FIELD)?
    } else {
        iterable
    };
    if dictionary.kind() != IDENTIFIER || !is_known(dictionary, Known::Dict, source) {
        return None;
    }
    let body = statement.child_by_field_name(BODY_FIELD)?;
    let mut pending = named(body);
    let mut indexed = false;
    while let Some(node) = pending.pop() {
        if node.kind() == SUBSCRIPT && is_load_subscript(node) {
            let of_dictionary = node.child_by_field_name(VALUE_FIELD).is_some_and(|value| {
                value.kind() == IDENTIFIER && text(value, source) == text(dictionary, source)
            });
            indexed |= of_dictionary && indexes_with(node, target, source);
            continue;
        }
        pending.extend(named(node));
    }
    indexed.then(|| Report::at(PythonRule::PLC0206, target, ""))
}
fn indexes_with(subscript: Node<'_>, target: Node<'_>, source: &[u8]) -> bool {
    let mut indexes: Vec<Node<'_>> = significant(subscript).into_iter().skip(1).collect();
    if let [only] = indexes.as_slice()
        && unparenthesized(*only).kind() == TUPLE
    {
        indexes = significant(unparenthesized(*only));
    }
    let targets = if target.kind() == IDENTIFIER {
        vec![target]
    } else {
        significant(target)
    };
    indexes.len() == targets.len()
        && indexes
            .iter()
            .zip(&targets)
            .all(|(index, name)| equivalent(*index, *name, source))
}
// RUF051: `if key in d: del d[key]`, que es `d.pop(key, None)`. Avisa en el `del`.
fn key_in_dict_del(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let condition = unparenthesized(statement.child_by_field_name(CONDITION_FIELD)?);
    let parts_3 = significant(condition);
    let [key, dictionary] = parts_3.as_slice() else {
        return None;
    };
    let mut cursor = condition.walk();
    let operators: Vec<&str> = condition
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    if condition.kind() != COMPARISON || operators != [IN] || !is_known(*dictionary, Known::Dict, source) {
        return None;
    }
    let body = significant(statement.child_by_field_name(CONSEQUENCE_FIELD)?);
    let [delete] = body.as_slice() else {
        return None;
    };
    let parts_4 = significant(*delete);
    let [target] = parts_4.as_slice() else {
        return None;
    };
    let deletes = delete.kind() == DELETE
        && target.kind() == SUBSCRIPT
        && target
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| equivalent(value, *dictionary, source))
        && matches!(significant(*target).as_slice(), [_, index] if equivalent(*index, *key, source));
    deletes.then(|| Report::at(PythonRule::RUF051, *delete, ""))
}
// PERF402: `for x in items: result.append(x)`, que es `result = list(items)`. Avisa en la llamada.
fn manual_list_copy(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement
        .child(0)
        .is_some_and(|first| text(first, source) == ASYNC)
    {
        return None;
    }
    let target = statement
        .child_by_field_name(LEFT_FIELD)
        .filter(|target| target.kind() == IDENTIFIER)?;
    let body = significant(statement.child_by_field_name(BODY_FIELD)?);
    let [only] = body.as_slice() else {
        return None;
    };
    let call = significant(*only)
        .first()
        .copied()
        .filter(|call| only.kind() == EXPRESSION_STATEMENT && call.kind() == CALL)?;
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let list = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|list| list.kind() == IDENTIFIER)?;
    let arguments = significant(call.child_by_field_name(ARGUMENTS_FIELD)?);
    let copies = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == APPEND)
        && matches!(arguments.as_slice(), [argument] if argument.kind() == IDENTIFIER && text(*argument, source) == text(target, source))
        && text(list, source) != text(target, source)
        && is_known(list, Known::List, source);
    copies.then(|| Report::at(PythonRule::PERF402, call, ""))
}
