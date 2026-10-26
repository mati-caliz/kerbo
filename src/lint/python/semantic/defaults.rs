use tree_sitter::Node;

use super::default_fixes::mutable_default_fix;
use super::mutability::Resolver;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    CALL, FUNCTION_FIELD, LAMBDA, TYPE_FIELD, VALUE_FIELD, named, unparenthesized,
};
use crate::rules::PythonRule;

const DEFAULTED: [&str; 2] = ["default_parameter", "typed_default_parameter"];

// B006 (un default mutable, sólo en funciones fuera de stubs) y B008 (una llamada en un default).
pub fn default_reports(
    resolver: &Resolver<'_, '_>,
    parameters: &[Node<'_>],
    checks_mutable: bool,
) -> Vec<Report> {
    let mut reports = Vec::new();
    for parameter in parameters
        .iter()
        .filter(|parameter| DEFAULTED.contains(&parameter.kind()))
    {
        let Some(default) = parameter.child_by_field_name(VALUE_FIELD) else {
            continue;
        };
        let immutable_annotation = parameter
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|annotation| resolver.is_immutable_annotation(annotation));
        if immutable_annotation {
            continue;
        }
        if checks_mutable && resolver.is_mutable_expression(default) {
            let fix = mutable_default_fix(*parameter, default, resolver.source);
            reports.push(Report::at(PythonRule::B006, unparenthesized(default), "").with_fix(fix));
        }
        reports.extend(
            calls_outside_lambdas(default)
                .into_iter()
                .filter(|call| is_evaluated_call(resolver, *call))
                .map(|call| Report::at(PythonRule::B008, call, "")),
        );
    }
    reports
}

fn is_evaluated_call(resolver: &Resolver<'_, '_>, call: Node<'_>) -> bool {
    call.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
        !resolver.is_mutable_function(function)
            && !resolver.is_immutable_function(function)
            && !resolver.is_immutable_new_type(function)
    })
}

fn calls_outside_lambdas(expression: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if node.kind() == LAMBDA {
            continue;
        }
        if node.kind() == CALL {
            found.push(node);
        }
        pending.extend(named(node));
    }
    found
}
