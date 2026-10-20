mod clippy;

use tree_sitter::Node;

use crate::macro_rules::MacroScope;
use crate::profile::NestingStyle;
use crate::syntax::one_based;

const ESLINT_STRUCTURES: &[&str] = &[
    "if_statement",
    "switch_statement",
    "try_statement",
    "do_statement",
    "while_statement",
    "with_statement",
    "for_statement",
    "for_in_statement",
];
const ESLINT_BOUNDARIES: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
    "method_definition",
    "class_static_block",
];
const IF_KIND: &str = "if_statement";
const ELSE_WRAPPER: &str = "else_clause";
const GO_FOR: &str = "for_statement";
const GO_RANGE: &str = "range_clause";
const GO_CASES: &[&str] = &[
    "expression_case",
    "type_case",
    "default_case",
    "communication_case",
];
const GO_FUNCTION_LITERAL: &str = "func_literal";
const GO_STATEMENT_LIST: &str = "statement_list";
const GO_COMMENT: &str = "comment";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NestingFinding {
    pub line: usize,
    pub column: usize,
    pub depth: usize,
}

pub fn find_deep_nesting(
    root: Node<'_>,
    style: NestingStyle,
    limit: Option<usize>,
    scope: Option<&MacroScope<'_>>,
) -> Vec<NestingFinding> {
    let Some(limit) = limit else {
        return Vec::new();
    };
    match style {
        NestingStyle::Unmeasured => Vec::new(),
        NestingStyle::Blocks => block_nesting(root, limit),
        NestingStyle::ControlFlow => control_nesting(root, limit),
        NestingStyle::Clippy => scope
            .map(|scope| clippy::reported(root, limit, scope))
            .unwrap_or_default(),
    }
}

fn position(node: Node<'_>, depth: usize) -> NestingFinding {
    let start = node.start_position();
    NestingFinding {
        line: one_based(start.row),
        column: one_based(start.column),
        depth,
    }
}

#[derive(Debug, Clone, Copy)]
enum BlockStep<'tree> {
    Enter(Node<'tree>),
    LeaveStructure,
    LeaveBoundary,
}

fn block_nesting(root: Node<'_>, limit: usize) -> Vec<NestingFinding> {
    let mut findings = Vec::new();
    let mut counters: Vec<isize> = vec![0];
    let mut steps = vec![BlockStep::Enter(root)];
    while let Some(step) = steps.pop() {
        match step {
            BlockStep::LeaveStructure => adjust(&mut counters, -1),
            BlockStep::LeaveBoundary => {
                counters.pop();
            }
            BlockStep::Enter(node) => {
                let kind = node.kind();
                if ESLINT_BOUNDARIES.contains(&kind) {
                    counters.push(0);
                    steps.push(BlockStep::LeaveBoundary);
                } else if ESLINT_STRUCTURES.contains(&kind) {
                    findings.extend(enter_structure(node, &mut counters, limit));
                    steps.push(BlockStep::LeaveStructure);
                }
                steps.extend(
                    (0..node.child_count())
                        .rev()
                        .filter_map(|index| node.child(index))
                        .map(BlockStep::Enter),
                );
            }
        }
    }
    findings
}

fn enter_structure(node: Node<'_>, counters: &mut [isize], limit: usize) -> Option<NestingFinding> {
    if continues_if(node) {
        return None;
    }
    adjust(counters, 1);
    let depth = counters.last().copied().unwrap_or_default();
    usize::try_from(depth)
        .ok()
        .filter(|&depth| depth > limit)
        .map(|depth| position(node, depth))
}

const fn adjust(counters: &mut [isize], delta: isize) {
    if let Some(counter) = counters.last_mut() {
        *counter = counter.saturating_add(delta);
    }
}

fn continues_if(node: Node<'_>) -> bool {
    node.kind() == IF_KIND
        && node.parent().is_some_and(|parent| {
            parent.kind() == IF_KIND
                || (parent.kind() == ELSE_WRAPPER
                    && parent.parent().is_some_and(|owner| owner.kind() == IF_KIND))
        })
}

#[derive(Debug, Clone, Copy)]
struct ControlVisit<'tree> {
    node: Node<'tree>,
    level: usize,
    walker: usize,
}

fn control_nesting(root: Node<'_>, limit: usize) -> Vec<NestingFinding> {
    let mut findings = Vec::new();
    let mut last_control: Vec<Option<Node<'_>>> = vec![None];
    let mut pending = vec![ControlVisit {
        node: root,
        level: 0,
        walker: 0,
    }];
    while let Some(visit) = pending.pop() {
        let ControlVisit { node, level, walker } = visit;
        if level > limit {
            let reported = last_control.get(walker).copied().flatten().unwrap_or(node);
            findings.push(position(reported, level));
            continue;
        }
        let deeper = level.saturating_add(1);
        let next: Vec<ControlVisit<'_>> = match node.kind() {
            IF_KIND => {
                set_last(&mut last_control, walker, node);
                ["consequence", "alternative"]
                    .into_iter()
                    .filter_map(|field| node.child_by_field_name(field))
                    .map(|child| visit_at(child, deeper, walker))
                    .collect()
            }
            GO_FOR if !has_child(node, GO_RANGE) => {
                set_last(&mut last_control, walker, node);
                node.child_by_field_name("body")
                    .map(|body| visit_at(body, deeper, walker))
                    .into_iter()
                    .collect()
            }
            kind if GO_CASES.contains(&kind) => {
                set_last(&mut last_control, walker, node);
                case_statements(node)
                    .into_iter()
                    .map(|statement| visit_at(statement, deeper, walker))
                    .collect()
            }
            GO_FUNCTION_LITERAL => {
                last_control.push(None);
                let literal_walker = last_control.len().saturating_sub(1);
                node.child_by_field_name("body")
                    .map(|body| ControlVisit {
                        node: body,
                        level: 0,
                        walker: literal_walker,
                    })
                    .into_iter()
                    .collect()
            }
            _ => (0..node.child_count())
                .filter_map(|index| node.child(index))
                .map(|child| visit_at(child, level, walker))
                .collect(),
        };
        pending.extend(next.into_iter().rev());
    }
    findings
}

const fn visit_at(node: Node<'_>, level: usize, walker: usize) -> ControlVisit<'_> {
    ControlVisit { node, level, walker }
}

fn set_last<'tree>(last_control: &mut [Option<Node<'tree>>], walker: usize, node: Node<'tree>) {
    if let Some(slot) = last_control.get_mut(walker) {
        *slot = Some(node);
    }
}

fn has_child(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| child.kind() == kind)
}

fn case_statements(case: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = case.walk();
    case.named_children(&mut cursor)
        .filter(|child| child.kind() == GO_STATEMENT_LIST)
        .flat_map(|list| {
            (0..list.named_child_count()).filter_map(move |index| {
                u32::try_from(index)
                    .ok()
                    .and_then(|index| list.named_child(index))
            })
        })
        .filter(|statement| statement.kind() != GO_COMMENT)
        .collect()
}

#[cfg(test)]
mod tests;
