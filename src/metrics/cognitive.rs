use tree_sitter::Node;

use crate::profile::Profile;
use crate::syntax::{children_with_fields, logical_operator, parent_skipping_parentheses};

const ALTERNATIVE_FIELD: &str = "alternative";
const CONDITION_FIELD: &str = "condition";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const UNNESTED_FIELDS: &[&str] = &[
    CONDITION_FIELD,
    "initializer",
    "increment",
    "update",
    LEFT_FIELD,
    RIGHT_FIELD,
    "value",
    "subject",
];
const STRUCTURE_INCREMENT: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Plain,
    ElseIf,
    Else,
}

#[derive(Debug, Clone, Copy)]
struct Visit<'tree> {
    node: Node<'tree>,
    nesting: usize,
    role: Role,
}

#[derive(Debug)]
struct Sequence<'source> {
    operators: Vec<&'source str>,
    has_conditional_operand: bool,
}

#[derive(Debug, Clone, Copy)]
enum SequenceItem<'tree, 'source> {
    Expand(Node<'tree>),
    Emit(&'source str),
}

struct Walker<'tree, 'context> {
    profile: &'context Profile,
    source: &'context [u8],
    pending: Vec<Visit<'tree>>,
    leaked_nesting: usize,
}

pub(super) fn cognitive(unit: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    let mut walker = Walker {
        profile,
        source,
        pending: Vec::new(),
        leaked_nesting: 0,
    };
    walker.push_children(unit, 0);
    let mut score: usize = 0;
    while let Some(visit) = walker.pending.pop() {
        score = score.saturating_add(walker.visit(visit));
    }
    score
}

impl<'tree> Walker<'tree, '_> {
    fn visit(&mut self, visit: Visit<'tree>) -> usize {
        let Visit { node, nesting, role } = visit;
        let shape = &self.profile.cognitive;
        let kind = node.kind();
        match role {
            Role::Else => {
                self.push_children(node, nesting.saturating_add(1));
                return STRUCTURE_INCREMENT;
            }
            Role::ElseIf => {
                self.push_if_children(node, nesting);
                return STRUCTURE_INCREMENT;
            }
            Role::Plain => {}
        }
        if self.profile.units.contains(&kind) {
            self.leaked_nesting = self
                .leaked_nesting
                .saturating_add(usize::from(self.leaks_nesting(node)));
            return 0;
        }
        if self.profile.inline_functions.contains(&kind) {
            self.push_children(node, nesting.saturating_add(1));
            return 0;
        }
        if kind == shape.if_kind {
            self.push_if_children(node, nesting);
            return self.structural_increment(nesting);
        }
        if shape.nesting.contains(&kind) {
            self.push_nested_structure_children(node, nesting);
            return self.structural_increment(nesting);
        }
        if shape.flat.contains(&kind) {
            self.push_children(node, nesting.saturating_add(1));
            return STRUCTURE_INCREMENT;
        }
        self.push_children(node, nesting);
        self.flat_increment(node)
    }

    fn flat_increment(&self, node: Node<'_>) -> usize {
        if self.profile.cognitive.labeled_jumps.contains(&node.kind()) {
            return usize::from(node.named_child_count() > 0);
        }
        if self.sequence_operator(node).is_none() || self.continues_sequence(node) {
            return 0;
        }
        let sequence = self.flatten_sequence(node);
        if self.is_short_circuit_render(node, &sequence) {
            return 0;
        }
        let cognitive_operators = self.profile.logical.cognitive_operators;
        let mut previous = None;
        let mut increments = 0_usize;
        for operator in sequence.operators {
            if cognitive_operators.contains(&operator) && previous != Some(operator) {
                increments = increments.saturating_add(1);
            }
            previous = Some(operator);
        }
        increments
    }

    fn sequence_operator<'source>(&'source self, node: Node<'_>) -> Option<&'source str> {
        logical_operator(
            self.profile,
            node,
            self.source,
            self.profile.logical.sequence_operators,
        )
    }

    fn continues_sequence(&self, node: Node<'_>) -> bool {
        parent_skipping_parentheses(self.profile, node)
            .is_some_and(|parent| self.sequence_operator(parent).is_some())
    }

    fn flatten_sequence(&self, root: Node<'tree>) -> Sequence<'_> {
        let mut sequence = Sequence {
            operators: Vec::new(),
            has_conditional_operand: false,
        };
        let mut pending = vec![SequenceItem::Expand(root)];
        while let Some(item) = pending.pop() {
            match item {
                SequenceItem::Emit(operator) => sequence.operators.push(operator),
                SequenceItem::Expand(node) => self.expand_sequence_node(node, &mut pending, &mut sequence),
            }
        }
        sequence
    }

    fn expand_sequence_node<'source>(
        &'source self,
        node: Node<'tree>,
        pending: &mut Vec<SequenceItem<'tree, 'source>>,
        sequence: &mut Sequence<'source>,
    ) {
        if self.profile.parenthesized.contains(&node.kind()) {
            pending.extend(node.named_child(0).map(SequenceItem::Expand));
            return;
        }
        let Some(operator) = self.sequence_operator(node) else {
            let conditional = self
                .profile
                .cognitive
                .short_circuit
                .as_ref()
                .map(|short| short.conditional);
            sequence.has_conditional_operand |= conditional == Some(node.kind());
            return;
        };
        pending.extend(node.child_by_field_name(RIGHT_FIELD).map(SequenceItem::Expand));
        pending.push(SequenceItem::Emit(operator));
        pending.extend(node.child_by_field_name(LEFT_FIELD).map(SequenceItem::Expand));
    }

    fn is_short_circuit_render(&self, node: Node<'_>, sequence: &Sequence<'_>) -> bool {
        let Some(short_circuit) = &self.profile.cognitive.short_circuit else {
            return false;
        };
        let rendered = parent_skipping_parentheses(self.profile, node)
            .is_some_and(|parent| parent.kind() == short_circuit.container);
        let single_operator = sequence
            .operators
            .windows(2)
            .all(|pair| pair.first() == pair.last());
        rendered && single_operator && !sequence.has_conditional_operand
    }

    fn push(&mut self, node: Node<'tree>, nesting: usize, role: Role) {
        self.pending.push(Visit { node, nesting, role });
    }

    const fn structural_increment(&self, nesting: usize) -> usize {
        STRUCTURE_INCREMENT
            .saturating_add(nesting)
            .saturating_add(self.leaked_nesting)
    }

    fn leaks_nesting(&self, function: Node<'_>) -> bool {
        let Some(leaking_parent) = self.profile.cognitive.leaking_branch_parent else {
            return false;
        };
        parent_skipping_parentheses(self.profile, function).is_some_and(|parent| {
            parent.kind() == leaking_parent
                && !parent
                    .child_by_field_name(CONDITION_FIELD)
                    .is_some_and(|condition| {
                        condition.start_byte() <= function.start_byte()
                            && function.end_byte() <= condition.end_byte()
                    })
        })
    }

    fn push_children(&mut self, node: Node<'tree>, nesting: usize) {
        for child in (0..node.child_count())
            .rev()
            .filter_map(|index| node.child(index))
        {
            self.push(child, nesting, Role::Plain);
        }
    }

    fn push_nested_structure_children(&mut self, node: Node<'tree>, nesting: usize) {
        for (field, child) in children_with_fields(node).into_iter().rev() {
            let unnested = field.is_some_and(|field| UNNESTED_FIELDS.contains(&field));
            let child_nesting = if unnested {
                nesting
            } else {
                nesting.saturating_add(1)
            };
            self.push(child, child_nesting, Role::Plain);
        }
    }

    fn push_if_children(&mut self, node: Node<'tree>, nesting: usize) {
        for (field, child) in children_with_fields(node).into_iter().rev() {
            match field {
                Some(ALTERNATIVE_FIELD) => self.push_alternative(child, nesting),
                Some(CONDITION_FIELD) => self.push(child, nesting, Role::Plain),
                _ => self.push(child, nesting.saturating_add(1), Role::Plain),
            }
        }
    }

    fn push_alternative(&mut self, alternative: Node<'tree>, nesting: usize) {
        let shape = &self.profile.cognitive;
        let kind = alternative.kind();
        if kind == shape.if_kind {
            self.push(alternative, nesting, Role::ElseIf);
        } else if shape.else_wrapper == Some(kind) {
            match alternative
                .named_child(0)
                .filter(|inner| inner.kind() == shape.if_kind)
            {
                Some(else_if) => self.push(else_if, nesting, Role::ElseIf),
                None => self.push(alternative, nesting, Role::Else),
            }
        } else if shape.flat.contains(&kind) {
            self.push(alternative, nesting, Role::Plain);
        } else {
            self.push(alternative, nesting, Role::Else);
        }
    }
}
