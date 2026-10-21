use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, CLASS, FUNCTION, IDENTIFIER, LEFT_FIELD, NAME_FIELD, PARENTHESIZED,
    named, text,
};
use crate::rules::PythonRule;

const AMBIGUOUS: [&str; 3] = ["l", "I", "O"];
const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_clause";
const AS_TARGET: &str = "as_pattern_target";
const NAMED_EXPRESSION: &str = "named_expression";
const PARAMETERS: [&str; 2] = ["parameters", "lambda_parameters"];
const DECLARATIONS: [&str; 2] = ["global_statement", "nonlocal_statement"];
const DESTRUCTURING: [&str; 8] = [
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "tuple",
    "list",
    "list_splat_pattern",
    "list_splat",
    PARENTHESIZED,
];
const NAMED_PARAMETERS: [&str; 2] = ["default_parameter", "typed_default_parameter"];
const TYPED_PARAMETER: &str = "typed_parameter";
const SPLATS: [&str; 2] = ["list_splat_pattern", "dictionary_splat_pattern"];

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    for node in nodes {
        let mut names = Vec::new();
        match node.kind() {
            ASSIGNMENT | AUGMENTED_ASSIGNMENT | FOR | FOR_IN => {
                if let Some(target) = node.child_by_field_name(LEFT_FIELD) {
                    targets(target, &mut names);
                }
            }
            AS_TARGET => named(*node)
                .into_iter()
                .for_each(|target| targets(target, &mut names)),
            NAMED_EXPRESSION => names.extend(node.child_by_field_name(NAME_FIELD)),
            kind if PARAMETERS.contains(&kind) => names.extend(parameter_names(*node)),
            kind if DECLARATIONS.contains(&kind) => names.extend(named(*node)),
            FUNCTION => reports.extend(definition(*node, PythonRule::E743, source)),
            CLASS => reports.extend(definition(*node, PythonRule::E742, source)),
            _ => {}
        }
        reports.extend(
            names
                .into_iter()
                .filter(|name| name.kind() == IDENTIFIER && is_ambiguous(*name, source))
                .map(|name| Report::at(PythonRule::E741, name, text(name, source))),
        );
    }
    reports
}

fn is_ambiguous(name: Node<'_>, source: &[u8]) -> bool {
    AMBIGUOUS.contains(&text(name, source))
}

fn definition(node: Node<'_>, rule: PythonRule, source: &[u8]) -> Option<Report> {
    node.child_by_field_name(NAME_FIELD)
        .filter(|name| is_ambiguous(*name, source))
        .map(|name| Report::at(rule, name, text(name, source)))
}

fn targets<'tree>(target: Node<'tree>, names: &mut Vec<Node<'tree>>) {
    if DESTRUCTURING.contains(&target.kind()) {
        for inner in named(target) {
            targets(inner, names);
        }
    } else {
        names.push(target);
    }
}

fn parameter_names(parameters: Node<'_>) -> Vec<Node<'_>> {
    named(parameters).into_iter().filter_map(parameter_name).collect()
}

fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    match parameter.kind() {
        kind if NAMED_PARAMETERS.contains(&kind) => parameter.child_by_field_name(NAME_FIELD),
        TYPED_PARAMETER => named(parameter).first().copied().and_then(parameter_name),
        kind if SPLATS.contains(&kind) => named(parameter)
            .into_iter()
            .find(|inner| inner.kind() == IDENTIFIER),
        _ => Some(parameter),
    }
}
