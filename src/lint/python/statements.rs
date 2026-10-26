use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, CALL, CLASS, EXPRESSION_STATEMENT, FUNCTION, FUNCTION_FIELD, IDENTIFIER, IMPORT, IMPORT_FROM,
    LAMBDA, LEFT_FIELD, NAME_FIELD, RIGHT_FIELD, TUPLE, TYPE_FIELD, WILDCARD_IMPORT, children, fields, named,
    text, unparenthesized,
};
use super::lambda_fix::lambda_to_def_fix;
use super::split_imports::split_imports_fix;
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
const TARGET_SEQUENCES: [&str; 3] = ["pattern_list", "tuple_pattern", "list_pattern"];
const AS_PATTERN_TARGET: &str = "as_pattern_target";
const STARRED_TARGETS: [&str; 2] = ["list_splat_pattern", "list_splat"];
const MAX_STARRED_TARGETS: usize = 1;

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            IMPORT => multiple_imports(*node, source),
            IMPORT_FROM => star_import(*node, source),
            TRY => handlers(*node),
            EXPRESSION_STATEMENT => lambda_assignment(*node, source),
            ASSERT => assert_tuple(*node),
            IF | ELIF => if_tuple(*node),
            PRINT => print_chevron(*node),
            RAISE => raise_not_implemented(*node, source),
            kind if TARGET_SEQUENCES.contains(&kind) => multiple_starred(*node),
            AS_PATTERN_TARGET => std::iter::once(*node)
                .chain(named(*node).into_iter().map(unparenthesized))
                .flat_map(multiple_starred)
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

// F622: `*a, *b = x`, más de un `*` en la misma tupla o lista de destinos.
fn multiple_starred(targets: Node<'_>) -> Vec<Report> {
    let starred = named(targets)
        .into_iter()
        .filter(|target| STARRED_TARGETS.contains(&target.kind()))
        .count();
    if starred > MAX_STARRED_TARGETS {
        vec![Report::at(PythonRule::F622, targets, "")]
    } else {
        Vec::new()
    }
}

fn multiple_imports(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    if fields(statement, NAME_FIELD).len() > 1 {
        vec![Report::at(PythonRule::E401, statement, "").with_fix(split_imports_fix(statement, source))]
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

fn lambda_assignment(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
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
        let fix = lambda_to_def_fix(statement, assignment, source);
        vec![Report::at(PythonRule::E731, assignment, "").with_fix(fix)]
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
