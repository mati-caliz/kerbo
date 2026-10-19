mod cognitive;
mod cognitive_pmd;
mod cyclomatic;
pub mod nesting;
mod parameters;

use tree_sitter::Node;

use crate::lines::LineMap;
use crate::profile::{Profile, SpanBounds};
use crate::syntax::{function_name, one_based, parent_skipping_parentheses};

const CALL_KIND: &str = "call_expression";
const CALLEE_FIELD: &str = "function";

#[derive(Debug)]
pub struct FunctionMetrics {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub lines: Option<usize>,
    pub cyclomatic: Option<usize>,
    pub cognitive: Option<usize>,
    pub parameters: Option<usize>,
}

struct Context<'context> {
    source: &'context [u8],
    profile: &'context Profile,
    line_map: &'context LineMap,
}

pub fn measure_functions(
    root: Node<'_>,
    source: &[u8],
    profile: &Profile,
    line_map: &LineMap,
) -> Vec<FunctionMetrics> {
    let context = Context {
        source,
        profile,
        line_map,
    };
    let mut functions = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        functions.extend(measure_unit(node, &context));
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    functions.sort_by_key(|function| (function.line, function.column));
    functions
}

fn measure_unit(node: Node<'_>, context: &Context<'_>) -> Option<FunctionMetrics> {
    let Context {
        source,
        profile,
        line_map,
    } = *context;
    let kind = node.kind();
    if profile.units.contains(&kind) {
        return Some(FunctionMetrics {
            lines: if skips_function_length(node, profile) {
                None
            } else {
                function_lines(node, profile, line_map)
            },
            cyclomatic: Some(cyclomatic::cyclomatic(node, profile, source)),
            cognitive: cognitive_of(node, profile, source),
            parameters: parameters::parameter_count(node, profile, source),
            ..unmeasured(node, source)
        });
    }
    if profile.partial_units.signature_only.contains(&kind) {
        return Some(FunctionMetrics {
            parameters: parameters::parameter_count(node, profile, source),
            ..unmeasured(node, source)
        });
    }
    let &(_, root_field) = profile
        .partial_units
        .complexity_only
        .iter()
        .find(|(partial, _)| *partial == kind)?;
    let cyclomatic = match root_field {
        Some(field) => {
            cyclomatic::cyclomatic_including_root(node.child_by_field_name(field)?, profile, source)
        }
        None => cyclomatic::cyclomatic(node, profile, source),
    };
    Some(FunctionMetrics {
        cyclomatic: Some(cyclomatic),
        ..unmeasured(node, source)
    })
}

fn skips_function_length(node: Node<'_>, profile: &Profile) -> bool {
    profile.function_length_skips_iife
        && parent_skipping_parentheses(profile, node).is_some_and(|parent| {
            parent.kind() == CALL_KIND
                && parent.child_by_field_name(CALLEE_FIELD).is_some_and(|callee| {
                    callee.start_byte() <= node.start_byte() && node.end_byte() <= callee.end_byte()
                })
        })
}

fn unmeasured(node: Node<'_>, source: &[u8]) -> FunctionMetrics {
    let start = node.start_position();
    FunctionMetrics {
        name: function_name(node, source),
        line: one_based(start.row),
        column: one_based(start.column),
        end_line: one_based(node.end_position().row),
        lines: None,
        cyclomatic: None,
        cognitive: None,
        parameters: None,
    }
}

fn function_lines(unit: Node<'_>, profile: &Profile, line_map: &LineMap) -> Option<usize> {
    let span = profile.function_lines;
    let first_row = match span.bounds {
        SpanBounds::Whole | SpanBounds::Interior => first_attached_row(unit, profile),
        SpanBounds::FromBody => unit.child_by_field_name("body")?.start_position().row,
    };
    Some(line_map.count(first_row, unit.end_position().row, span))
}

fn cognitive_of(unit: Node<'_>, profile: &Profile, source: &[u8]) -> Option<usize> {
    let shape = &profile.cognitive;
    if shape.skipped_units.contains(&unit.kind()) {
        return None;
    }
    Some(shape.pmd.as_ref().map_or_else(
        || cognitive::cognitive(unit, profile, source),
        |pmd| cognitive_pmd::cognitive(unit, profile, pmd, source),
    ))
}

fn first_attached_row(unit: Node<'_>, profile: &Profile) -> usize {
    let mut first = unit;
    while let Some(previous) = first
        .prev_sibling()
        .filter(|previous| profile.leading_attachments.contains(&previous.kind()))
    {
        first = previous;
    }
    first.start_position().row
}

pub fn body_descendants<'tree>(unit: Node<'tree>, profile: &Profile) -> Vec<Node<'tree>> {
    let mut descendants = Vec::new();
    let mut cursor = unit.walk();
    let mut pending: Vec<Node<'tree>> = unit.children(&mut cursor).collect();
    while let Some(node) = pending.pop() {
        if is_separate_unit(node, profile) {
            continue;
        }
        descendants.push(node);
        let mut child_cursor = node.walk();
        pending.extend(node.children(&mut child_cursor));
    }
    descendants
}

fn is_separate_unit(node: Node<'_>, profile: &Profile) -> bool {
    let kind = node.kind();
    profile.units.contains(&kind)
        || profile
            .partial_units
            .complexity_only
            .iter()
            .any(|(partial, _)| *partial == kind)
}

#[cfg(test)]
mod tests;
