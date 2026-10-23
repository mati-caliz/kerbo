use std::collections::{HashMap, HashSet, VecDeque};

use tree_sitter::Node;

use super::nodes::{
    COMMENT, IDENTIFIER, LEFT_FIELD, NAME_FIELD, PROGRAM, RIGHT_FIELD, VALUE_FIELD, without_parentheses,
};
use super::paths::{Flow, PathRecord};
use super::scope::{DefinitionKind, ScopeId, ScopeKind, ScopeTree, Variable, VariableId};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const DECLARATOR: &str = "variable_declarator";
const UPDATE: &str = "update_expression";
const ARGUMENT_FIELD: &str = "argument";
const UNDEFINED: &str = "undefined";
const SHORTHAND_PATTERN: &str = "shorthand_property_identifier_pattern";
const PAIR_PATTERN: &str = "pair_pattern";
const OBJECT_ASSIGNMENT_PATTERN: &str = "object_assignment_pattern";
const ASSIGNMENT_PATTERN: &str = "assignment_pattern";
const OBJECT_PATTERN: &str = "object_pattern";
const ARRAY_PATTERN: &str = "array_pattern";
const REST_PATTERN: &str = "rest_pattern";
const EXPORT: &str = "export_statement";
const EXPORT_SPECIFIER: &str = "export_specifier";

// Una asignación a una variable: el identificador, el valor (si hay) y los segmentos activos.
#[derive(Clone)]
struct Target<'tree> {
    identifier: Node<'tree>,
    expression: Option<Node<'tree>>,
    segments: Vec<usize>,
}

// Lo que pasa después de una asignación: otra que la pisa en el mismo segmento y los segmentos que
// siguen, cada uno con la asignación que lo corta, si la hay.
struct After<'targets, 'tree> {
    other: Option<&'targets Target<'tree>>,
    subsequent: Vec<(usize, Option<Target<'tree>>)>,
}

struct Verifier<'flow, 'tree> {
    flow: &'flow Flow<'tree>,
    record: &'flow PathRecord<'tree>,
    tree: &'flow ScopeTree<'tree>,
    path_scope: Option<ScopeId>,
    start_scopes: &'flow HashSet<ScopeId>,
}

// `no-useless-assignment`: un valor asignado que ningún camino llega a leer antes de pisarlo.
pub fn useless_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let flow = context.paths();
    let tree = context.scopes();
    let resolved: HashMap<usize, VariableId> = tree
        .references
        .iter()
        .filter_map(|reference| {
            reference
                .resolved
                .map(|variable| (reference.identifier.id(), variable))
        })
        .collect();
    let start_scopes: HashSet<ScopeId> = flow
        .paths
        .iter()
        .filter_map(|record| path_scope(tree, record.start))
        .collect();
    let mut found = Vec::new();
    for record in &flow.paths {
        let verifier = Verifier {
            flow,
            record,
            tree,
            path_scope: path_scope(tree, record.start),
            start_scopes: &start_scopes,
        };
        for (variable, mut targets) in verifier.collect_targets(&resolved) {
            targets.sort_by_key(|target| target.identifier.start_byte());
            found.extend(
                targets
                    .iter()
                    .filter(|target| verifier.is_useless(variable, target, &targets))
                    .map(|target| {
                        report(
                            EslintRule::NoUselessAssignment,
                            target.identifier,
                            context.text(target.identifier),
                        )
                    }),
            );
        }
    }
    found
}

// `sourceCode.getScope` del nodo que abre el camino: el programa abre el alcance global.
fn path_scope(tree: &ScopeTree<'_>, start: Node<'_>) -> Option<ScopeId> {
    if start.kind() == PROGRAM {
        return tree
            .scopes
            .iter()
            .position(|scope| scope.kind == ScopeKind::Global);
    }
    Some(tree.scope_of(start))
}

impl<'tree> Verifier<'_, 'tree> {
    fn code_path_start_scope(&self, scope: ScopeId) -> Option<ScopeId> {
        let mut current = Some(scope);
        while let Some(id) = current {
            if self.start_scopes.contains(&id) {
                return Some(id);
            }
            current = self.tree.scope(id).and_then(|found| found.upper);
        }
        None
    }

    fn collect_targets(
        &self,
        resolved: &HashMap<usize, VariableId>,
    ) -> Vec<(VariableId, Vec<Target<'tree>>)> {
        let mut groups: Vec<(VariableId, Vec<Target<'tree>>)> = Vec::new();
        for assignment in &self.record.assignments {
            let (pattern, expression) = assignment_parts(assignment.node);
            let identifiers = pattern.map(pattern_identifiers).unwrap_or_default();
            for (identifier, variable) in identifiers
                .into_iter()
                .filter_map(|identifier| {
                    resolved
                        .get(&identifier.id())
                        .map(|variable| (identifier, *variable))
                })
                .filter(|(_, variable)| self.is_tracked(*variable))
            {
                let target = Target {
                    identifier,
                    expression,
                    segments: assignment.segments.clone(),
                };
                add_to_group(&mut groups, variable, target);
            }
        }
        groups
    }

    fn is_tracked(&self, id: VariableId) -> bool {
        let Some(variable) = self.tree.variable(id) else {
            return false;
        };
        if self.code_path_start_scope(variable.scope) != self.path_scope {
            return false;
        }
        let is_module = self
            .tree
            .scope(variable.scope)
            .is_some_and(|scope| scope.kind == ScopeKind::Module);
        !(is_module && is_exported(self.tree, variable))
    }

    fn is_useless(&self, id: VariableId, target: &Target<'tree>, all: &[Target<'tree>]) -> bool {
        let Some(variable) = self.tree.variable(id) else {
            return false;
        };
        if self
            .record
            .try_blocks
            .iter()
            .any(|block| contains(*block, target.identifier))
        {
            return false;
        }
        let reads: Vec<Node<'tree>> = self
            .tree
            .references_of(variable)
            .filter(|reference| reference.is_read())
            .map(|reference| reference.identifier)
            .collect();
        if reads.is_empty() {
            return false;
        }
        if self
            .tree
            .references_of(variable)
            .filter(|reference| reference.is_read())
            .any(|reference| self.code_path_start_scope(reference.from) != self.path_scope)
        {
            return false;
        }
        let after = After {
            other: all.iter().find(|other| is_overwriting(target, other)),
            subsequent: self.subsequent_segments(target, all),
        };
        !reads.iter().any(|read| self.is_read_used(target, &after, *read))
    }

    fn is_read_used(&self, target: &Target<'tree>, after: &After<'_, 'tree>, read: Node<'tree>) -> bool {
        let read_in_same_segment = evaluated_after(target, read)
            && (used_between(target, read)
                || target
                    .segments
                    .iter()
                    .any(|segment| self.used_in_segment(*segment, read)));
        if read_in_same_segment {
            return !after.other.is_some_and(|other| evaluated_after(other, read));
        }
        if after.other.is_some() {
            return false;
        }
        after
            .subsequent
            .iter()
            .filter(|(segment, _)| self.used_in_segment(*segment, read))
            .any(|(_, assignment)| {
                !assignment
                    .as_ref()
                    .is_some_and(|assignment| evaluated_after(assignment, read))
            })
    }

    fn used_in_segment(&self, segment: usize, identifier: Node<'_>) -> bool {
        self.record
            .spans
            .get(&segment)
            .copied()
            .flatten()
            .is_some_and(|span| {
                span.first.start_byte() <= identifier.start_byte()
                    && identifier.end_byte() <= span.last.end_byte()
            })
    }

    // `getSubsequentSegments`: los segmentos que siguen, sin pasar de uno que vuelve a asignar.
    fn subsequent_segments(
        &self,
        target: &Target<'tree>,
        all: &[Target<'tree>],
    ) -> Vec<(usize, Option<Target<'tree>>)> {
        let mut queue: VecDeque<usize> = target
            .segments
            .iter()
            .flat_map(|segment| self.flow.successors(*segment).iter().copied())
            .collect();
        let mut visited = HashSet::new();
        let mut results = Vec::new();
        while let Some(segment) = queue.pop_front() {
            if !visited.insert(segment) {
                continue;
            }
            let assignment = all
                .iter()
                .find(|other| other.segments.contains(&segment) && !used_between(other, target.identifier))
                .cloned();
            if assignment.is_none() {
                queue.extend(self.flow.successors(segment).iter().copied());
            }
            results.push((segment, assignment));
        }
        results
    }
}

fn is_overwriting(target: &Target<'_>, other: &Target<'_>) -> bool {
    if other.identifier.id() == target.identifier.id() {
        return false;
    }
    if !other.segments.is_empty()
        && other
            .segments
            .iter()
            .all(|segment| !target.segments.contains(segment))
    {
        return false;
    }
    evaluated_after(target, other.identifier)
        || other
            .expression
            .is_some_and(|expression| contains(expression, target.identifier))
}

// `isIdentifierEvaluatedAfterAssignment`.
fn evaluated_after(assignment: &Target<'_>, identifier: Node<'_>) -> bool {
    identifier.start_byte() >= assignment.identifier.end_byte()
        && !assignment
            .expression
            .is_some_and(|expression| contains(expression, identifier))
}

// `isIdentifierUsedBetweenAssignedAndEqualSign`.
fn used_between(assignment: &Target<'_>, identifier: Node<'_>) -> bool {
    assignment.expression.is_some_and(|expression| {
        assignment.identifier.end_byte() <= identifier.start_byte()
            && identifier.end_byte() <= expression.start_byte()
    })
}

fn contains(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn assignment_parts(node: Node<'_>) -> (Option<Node<'_>>, Option<Node<'_>>) {
    match node.kind() {
        DECLARATOR => (
            node.child_by_field_name(NAME_FIELD),
            node.child_by_field_name(VALUE_FIELD),
        ),
        UPDATE => (node.child_by_field_name(ARGUMENT_FIELD), None),
        _ => (
            node.child_by_field_name(LEFT_FIELD),
            node.child_by_field_name(RIGHT_FIELD),
        ),
    }
}

// `extractIdentifiersFromPattern`.
fn pattern_identifiers(pattern: Node<'_>) -> Vec<Node<'_>> {
    let pattern = without_parentheses(pattern);
    match pattern.kind() {
        IDENTIFIER | UNDEFINED | SHORTHAND_PATTERN => vec![pattern],
        PAIR_PATTERN => pattern
            .child_by_field_name(VALUE_FIELD)
            .map(pattern_identifiers)
            .unwrap_or_default(),
        OBJECT_ASSIGNMENT_PATTERN | ASSIGNMENT_PATTERN => pattern
            .child_by_field_name(LEFT_FIELD)
            .map(pattern_identifiers)
            .unwrap_or_default(),
        OBJECT_PATTERN | ARRAY_PATTERN | REST_PATTERN => {
            let mut cursor = pattern.walk();
            pattern
                .named_children(&mut cursor)
                .filter(|child| child.kind() != COMMENT)
                .flat_map(pattern_identifiers)
                .collect()
        }
        _ => Vec::new(),
    }
}

fn is_exported(tree: &ScopeTree<'_>, variable: &Variable<'_>) -> bool {
    let declared_exported = variable
        .definitions
        .iter()
        .any(|definition| match definition.kind {
            DefinitionKind::Variable => definition
                .declaration
                .and_then(|declaration| declaration.parent())
                .is_some_and(|parent| parent.kind() == EXPORT),
            DefinitionKind::FunctionName | DefinitionKind::ClassName => definition
                .node
                .parent()
                .is_some_and(|parent| parent.kind() == EXPORT),
            DefinitionKind::CatchClause
            | DefinitionKind::ImportBinding { .. }
            | DefinitionKind::Parameter
            | DefinitionKind::EnumMember
            | DefinitionKind::EnumName
            | DefinitionKind::ModuleName
            | DefinitionKind::Type => false,
        });
    declared_exported
        || tree.references_of(variable).any(|reference| {
            reference
                .identifier
                .parent()
                .is_some_and(|parent| parent.kind() == EXPORT_SPECIFIER)
        })
}

fn add_to_group<'tree>(
    groups: &mut Vec<(VariableId, Vec<Target<'tree>>)>,
    variable: VariableId,
    target: Target<'tree>,
) {
    match groups.iter_mut().find(|(known, _)| *known == variable) {
        Some((_, targets)) => targets.push(target),
        None => groups.push((variable, vec![target])),
    }
}
