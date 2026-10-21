use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, CALL, CLASS, EXPRESSION_STATEMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER, IMPORT, IMPORT_FROM,
    LAMBDA, LEFT_FIELD, NAME_FIELD, RIGHT_FIELD, TUPLE, TYPE_FIELD, WILDCARD_IMPORT, children, fields, named,
    text, unparenthesized,
};
use crate::fix::Fix;
use crate::rules::PythonRule;

const TRY: &str = "try_statement";
const EXCEPT: &str = "except_clause";
const RAISE: &str = "raise_statement";
const ASSERT: &str = "assert_statement";
const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const PRINT: &str = "print_statement";
const CHEVRON: &str = "chevron";
const CONDITION_FIELD: &str = "condition";
const NOT_IMPLEMENTED: &str = "NotImplemented";
const NOT_IMPLEMENTED_ERROR: &str = "NotImplementedError";

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            IMPORT => multiple_imports(*node),
            IMPORT_FROM => star_import(*node, source),
            TRY => handlers(*node),
            EXPRESSION_STATEMENT => lambda_assignment(*node),
            ASSERT => assert_tuple(*node),
            IF | ELIF => if_tuple(*node),
            PRINT => print_chevron(*node),
            RAISE => raise_not_implemented(*node, source),
            _ => Vec::new(),
        })
        .collect()
}

fn multiple_imports(statement: Node<'_>) -> Vec<Report> {
    if fields(statement, NAME_FIELD).len() > 1 {
        vec![Report::at(PythonRule::E401, statement, "")]
    } else {
        Vec::new()
    }
}

fn star_import(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    if !children(statement)
        .iter()
        .any(|child| child.kind() == WILDCARD_IMPORT)
    {
        return Vec::new();
    }
    let subject = text(statement, source);
    let mut reports = vec![Report::at(PythonRule::F403, statement, subject)];
    let nested = super::ast::statement_ancestors(statement)
        .any(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS));
    if nested {
        reports.push(Report::at(PythonRule::F406, statement, subject));
    }
    reports
}

fn handlers(statement: Node<'_>) -> Vec<Report> {
    let handlers: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|child| child.kind() == EXCEPT)
        .collect();
    let last = handlers.len().saturating_sub(1);
    let mut reports = Vec::new();
    for (index, handler) in handlers.iter().enumerate() {
        if !is_bare(*handler) {
            continue;
        }
        if index < last {
            reports.push(Report::at(PythonRule::F707, *handler, ""));
        }
        if !reraises(*handler) {
            reports.push(Report::at(PythonRule::E722, *handler, ""));
        }
    }
    reports
}

fn is_bare(handler: Node<'_>) -> bool {
    !named(handler)
        .iter()
        .any(|child| child.kind() != super::ast::BLOCK)
}

fn reraises(handler: Node<'_>) -> bool {
    named(handler)
        .iter()
        .filter(|child| child.kind() == super::ast::BLOCK)
        .flat_map(|block| named(*block))
        .any(|statement| statement.kind() == RAISE && named(statement).is_empty())
}

fn lambda_assignment(statement: Node<'_>) -> Vec<Report> {
    let Some(assignment) = named(statement)
        .into_iter()
        .find(|child| child.kind() == ASSIGNMENT)
    else {
        return Vec::new();
    };
    let (Some(target), Some(value)) = (
        assignment.child_by_field_name(LEFT_FIELD),
        assignment.child_by_field_name(RIGHT_FIELD),
    ) else {
        return Vec::new();
    };
    let annotated = assignment.child_by_field_name(TYPE_FIELD).is_some();
    let is_lambda = target.kind() == IDENTIFIER && unparenthesized(value).kind() == LAMBDA;
    if is_lambda && !(annotated && in_class_body(statement)) {
        vec![Report::at(PythonRule::E731, assignment, "")]
    } else {
        Vec::new()
    }
}

fn in_class_body(statement: Node<'_>) -> bool {
    super::ast::statement_ancestors(statement)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS))
        .is_some_and(|scope| scope.kind() == CLASS)
}

fn assert_tuple(statement: Node<'_>) -> Vec<Report> {
    named(statement)
        .first()
        .filter(|test| is_non_empty_tuple(**test))
        .map(|_| Report::at(PythonRule::F631, statement, ""))
        .into_iter()
        .collect()
}

fn if_tuple(clause: Node<'_>) -> Vec<Report> {
    clause
        .child_by_field_name(CONDITION_FIELD)
        .filter(|test| is_non_empty_tuple(*test))
        .map(|test| Report::at(PythonRule::F634, unparenthesized(test), ""))
        .into_iter()
        .collect()
}

fn is_non_empty_tuple(node: Node<'_>) -> bool {
    let node = unparenthesized(node);
    node.kind() == TUPLE && !named(node).is_empty()
}

fn print_chevron(statement: Node<'_>) -> Vec<Report> {
    if named(statement).iter().any(|child| child.kind() == CHEVRON) {
        vec![Report::at(PythonRule::F633, statement, "")]
    } else {
        Vec::new()
    }
}

fn raise_not_implemented(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(raised) = named(statement).first().copied() else {
        return Vec::new();
    };
    let name = if raised.kind() == CALL {
        raised.child_by_field_name(FUNCTION_FIELD)
    } else {
        Some(raised)
    };
    name.filter(|name| name.kind() == IDENTIFIER && text(*name, source) == NOT_IMPLEMENTED)
        .map(|name| {
            let fix = Fix::replace(name, NOT_IMPLEMENTED_ERROR, source);
            Report::at(PythonRule::F901, name, NOT_IMPLEMENTED).with_fix(Some(fix))
        })
        .into_iter()
        .collect()
}
