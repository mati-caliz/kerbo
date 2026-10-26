use tree_sitter::Node;

use super::Report;
use super::ast::{
    ALIAS_FIELD, BLOCK, CLASS, COMMENT, COMPARISON, DECORATED, FALSE, FUNCTION, IDENTIFIER, NAME_FIELD, TRUE,
    VALUE_FIELD, named, text, unparenthesized,
};
use super::codegen;
use crate::fix::Fix;
use crate::rules::PythonRule;

const BOOL: &str = "bool";

const TRY: &str = "try_statement";
const RAISE: &str = "raise_statement";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const AS_PATTERN: &str = "as_pattern";
const CAUSE_FIELD: &str = "cause";
const CONDITIONAL: &str = "conditional_expression";
const DECORATOR: &str = "decorator";
const EXIT_METHOD: &str = "__exit__";
const NON_METHOD_DECORATORS: [&str; 2] = ["staticmethod", "classmethod"];

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            TRY => useless_try_except(*node, source),
            RAISE => misplaced_bare_raise(*node, source).into_iter().collect(),
            CONDITIONAL => true_false_ternary(*node, source).into_iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}

// TRY203: un `try` cuyos `except` empiezan todos volviendo a lanzar la misma excepción, así que no
// manejan nada. Avisa en cada `except`.
fn useless_try_except(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let handlers: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|child| EXCEPT_CLAUSES.contains(&child.kind()))
        .collect();
    if !handlers.iter().all(|handler| reraises_first(*handler, source)) {
        return Vec::new();
    }
    handlers
        .into_iter()
        .map(|handler| Report::at(PythonRule::TRY203, handler, ""))
        .collect()
}

fn reraises_first(handler: Node<'_>, source: &[u8]) -> bool {
    let Some(first) = named(handler)
        .into_iter()
        .find(|child| child.kind() == BLOCK)
        .and_then(|block| {
            named(block)
                .into_iter()
                .find(|statement| statement.kind() != COMMENT)
        })
    else {
        return false;
    };
    if first.kind() != RAISE || first.child_by_field_name(CAUSE_FIELD).is_some() {
        return false;
    }
    let raised: Vec<Node<'_>> = named(first)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    match raised.as_slice() {
        [] => true,
        [value] => {
            let value = unparenthesized(*value);
            value.kind() == IDENTIFIER && handler_name(handler, source) == Some(text(value, source))
        }
        _ => false,
    }
}

fn handler_name<'source>(handler: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let value = handler
        .child_by_field_name(VALUE_FIELD)
        .filter(|value| value.kind() == AS_PATTERN)?;
    named(value.child_by_field_name(ALIAS_FIELD)?)
        .into_iter()
        .find(|inner| inner.kind() == IDENTIFIER)
        .map(|identifier| text(identifier, source))
}

// PLE0704: un `raise` a secas fuera de un `except` (una función definida adentro de uno ya no está
// adentro) y fuera del `__exit__` de una clase.
fn misplaced_bare_raise(raise: Node<'_>, source: &[u8]) -> Option<Report> {
    let is_bare = named(raise).iter().all(|child| child.kind() == COMMENT);
    let ancestors = std::iter::successors(raise.parent(), Node::parent);
    let in_handler = ancestors
        .take_while(|ancestor| ancestor.kind() != FUNCTION)
        .any(|ancestor| EXCEPT_CLAUSES.contains(&ancestor.kind()));
    (is_bare && !in_handler && !is_in_exit_method(raise, source))
        .then(|| Report::at(PythonRule::PLE0704, raise, ""))
}

fn is_in_exit_method(raise: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = std::iter::successors(raise.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS))
        .filter(|scope| scope.kind() == FUNCTION)
    else {
        return false;
    };
    let is_exit = function
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == EXIT_METHOD);
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    let is_plain_method = !named(holder)
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .any(|decorator| {
            NON_METHOD_DECORATORS.contains(&text(decorator, source).trim_start_matches('@').trim())
        });
    let in_class = std::iter::successors(holder.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS))
        .is_some_and(|scope| scope.kind() == CLASS);
    is_exit && is_plain_method && in_class
}

// SIM210: `True if condición else False`, que es `bool(condición)` (o la condición, si ya es una
// comparación).
fn true_false_ternary(expression: Node<'_>, source: &[u8]) -> Option<Report> {
    let parts: Vec<Node<'_>> = named(expression)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    let [body, test, alternative] = parts.as_slice() else {
        return None;
    };
    (unparenthesized(*body).kind() == TRUE && unparenthesized(*alternative).kind() == FALSE).then(|| {
        let replacement = if unparenthesized(*test).kind() == COMPARISON {
            text(*test, source).to_owned()
        } else {
            format!("{BOOL}({})", codegen::unparse(*test, codegen::COMMA, source))
        };
        let fix = Fix::replace(expression, &replacement, source).marked_unsafe();
        Report::at(PythonRule::SIM210, expression, "").with_fix(Some(fix))
    })
}
