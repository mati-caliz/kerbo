use tree_sitter::Node;

use crate::profile::Profile;
use crate::syntax::{children_with_fields, logical_operator, parent_skipping_parentheses};

const ALTERNATIVE_FIELD: &str = "alternative";
const CONDITION_FIELD: &str = "condition";
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

struct Walker<'tree, 'context> {
    profile: &'context Profile,
    source: &'context [u8],
    pending: Vec<Visit<'tree>>,
}

pub(super) fn cognitive(unit: Node<'_>, profile: &Profile, source: &[u8]) -> usize {
    let mut walker = Walker {
        profile,
        source,
        pending: Vec::new(),
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
            return 0;
        }
        if self.profile.inline_functions.contains(&kind) {
            self.push_children(node, nesting.saturating_add(1));
            return 0;
        }
        if kind == shape.if_kind {
            self.push_if_children(node, nesting);
            return STRUCTURE_INCREMENT.saturating_add(nesting);
        }
        if shape.nesting.contains(&kind) {
            self.push_nested_structure_children(node, nesting);
            return STRUCTURE_INCREMENT.saturating_add(nesting);
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
        let Some(operator) = logical_operator(
            self.profile,
            node,
            self.source,
            self.profile.logical.cognitive_operators,
        ) else {
            return 0;
        };
        let continues_sequence = parent_skipping_parentheses(self.profile, node)
            .and_then(|parent| {
                logical_operator(
                    self.profile,
                    parent,
                    self.source,
                    self.profile.logical.cognitive_operators,
                )
            })
            .is_some_and(|parent_operator| parent_operator == operator);
        usize::from(!continues_sequence)
    }

    fn push(&mut self, node: Node<'tree>, nesting: usize, role: Role) {
        self.pending.push(Visit { node, nesting, role });
    }

    fn push_children(&mut self, node: Node<'tree>, nesting: usize) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.push(child, nesting, Role::Plain);
        }
    }

    fn push_nested_structure_children(&mut self, node: Node<'tree>, nesting: usize) {
        for (field, child) in children_with_fields(node) {
            let child_nesting = if field == Some(CONDITION_FIELD) {
                nesting
            } else {
                nesting.saturating_add(1)
            };
            self.push(child, child_nesting, Role::Plain);
        }
    }

    fn push_if_children(&mut self, node: Node<'tree>, nesting: usize) {
        for (field, child) in children_with_fields(node) {
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
