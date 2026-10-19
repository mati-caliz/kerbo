mod cognitive;
mod cyclomatic;
mod parameters;

use tree_sitter::Node;

use crate::lines::LineMap;
use crate::profile::{Profile, SpanBounds};
use crate::syntax::{function_name, one_based};

#[derive(Debug)]
pub struct FunctionMetrics {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub lines: usize,
    pub cyclomatic: usize,
    pub cognitive: usize,
    pub parameters: Option<usize>,
}

pub fn measure_functions(
    root: Node<'_>,
    source: &[u8],
    profile: &Profile,
    line_map: &LineMap,
) -> Vec<FunctionMetrics> {
    let mut functions = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if profile.units.contains(&node.kind()) {
            functions.push(measure_function(node, source, profile, line_map));
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    functions.sort_by_key(|function| (function.line, function.column));
    functions
}

fn measure_function(unit: Node<'_>, source: &[u8], profile: &Profile, line_map: &LineMap) -> FunctionMetrics {
    let start = unit.start_position();
    FunctionMetrics {
        name: function_name(unit, source),
        line: one_based(start.row),
        column: one_based(start.column),
        lines: function_lines(unit, profile, line_map),
        cyclomatic: cyclomatic::cyclomatic(unit, profile, source),
        cognitive: cognitive::cognitive(unit, profile, source),
        parameters: parameters::parameter_count(unit, profile, source),
    }
}

fn function_lines(unit: Node<'_>, profile: &Profile, line_map: &LineMap) -> usize {
    let span = profile.function_lines;
    let first_row = match span.bounds {
        SpanBounds::Whole | SpanBounds::Interior => unit.start_position().row,
        SpanBounds::FromBody => {
            unit.child_by_field_name("body")
                .unwrap_or(unit)
                .start_position()
                .row
        }
    };
    line_map.count(first_row, unit.end_position().row, span)
}

pub fn body_descendants<'tree>(unit: Node<'tree>, profile: &Profile) -> Vec<Node<'tree>> {
    let mut descendants = Vec::new();
    let mut cursor = unit.walk();
    let mut pending: Vec<Node<'tree>> = unit.children(&mut cursor).collect();
    while let Some(node) = pending.pop() {
        if profile.units.contains(&node.kind()) {
            continue;
        }
        descendants.push(node);
        let mut child_cursor = node.walk();
        pending.extend(node.children(&mut child_cursor));
    }
    descendants
}

#[cfg(test)]
mod tests;
