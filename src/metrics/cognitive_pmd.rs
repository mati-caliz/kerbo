use tree_sitter::Node;

use crate::profile::{PmdShape, Profile};
use crate::syntax::logical_operator;

const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const NAME_FIELD: &str = "name";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const PARAMETERS_FIELD: &str = "parameters";

#[derive(Debug, Clone, Copy)]
enum Event<'tree> {
    Visit(Node<'tree>),
    Structural,
    Hybrid,
    IncreaseNesting,
    DecreaseNesting,
    EndBooleanRun,
}

#[derive(Debug)]
struct Recursion<'source> {
    name: &'source str,
    arity: usize,
}

struct PmdWalker<'tree, 'context> {
    profile: &'context Profile,
    shape: &'context PmdShape,
    source: &'context [u8],
    events: Vec<Event<'tree>>,
    complexity: usize,
    nesting: usize,
    boolean_run: Option<&'context str>,
    recursion: Option<Recursion<'context>>,
}

pub(super) fn cognitive(unit: Node<'_>, profile: &Profile, shape: &PmdShape, source: &[u8]) -> usize {
    let mut walker = PmdWalker {
        profile,
        shape,
        source,
        events: Vec::new(),
        complexity: 0,
        nesting: 0,
        boolean_run: None,
        recursion: recursion_target(unit, source),
    };
    walker.push_children(unit);
    while let Some(event) = walker.events.pop() {
        walker.apply(event);
    }
    walker.complexity
}

fn recursion_target<'source>(unit: Node<'_>, source: &'source [u8]) -> Option<Recursion<'source>> {
    let name = unit.child_by_field_name(NAME_FIELD)?.utf8_text(source).ok()?;
    let arity = arity_of(unit);
    let mut cursor = unit.parent()?.walk();
    let same_signature = unit
        .parent()?
        .named_children(&mut cursor)
        .filter(|sibling| sibling.kind() == unit.kind())
        .filter(|sibling| {
            sibling
                .child_by_field_name(NAME_FIELD)
                .and_then(|other| other.utf8_text(source).ok())
                == Some(name)
        })
        .filter(|&sibling| arity_of(sibling) == arity)
        .count();
    (same_signature == 1).then_some(Recursion { name, arity })
}

fn arity_of(unit: Node<'_>) -> usize {
    unit.child_by_field_name(PARAMETERS_FIELD)
        .map_or(0, |parameters| parameters.named_child_count())
}

impl<'tree, 'context> PmdWalker<'tree, 'context> {
    fn apply(&mut self, event: Event<'tree>) {
        match event {
            Event::Visit(node) => self.visit(node),
            Event::Structural => {
                self.complexity = self.complexity.saturating_add(1).saturating_add(self.nesting);
                self.nesting = self.nesting.saturating_add(1);
            }
            Event::Hybrid => {
                self.complexity = self.complexity.saturating_add(1);
                self.nesting = self.nesting.saturating_add(1);
            }
            Event::IncreaseNesting => self.nesting = self.nesting.saturating_add(1),
            Event::DecreaseNesting => self.nesting = self.nesting.saturating_sub(1),
            Event::EndBooleanRun => self.boolean_run = None,
        }
    }

    fn visit(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        if kind == self.profile.cognitive.if_kind {
            self.visit_if(node);
            return;
        }
        if self.shape.structural.contains(&kind) {
            self.wrap_children(node, Event::Structural);
            return;
        }
        if self.shape.nesting_only.contains(&kind) {
            self.wrap_children(node, Event::IncreaseNesting);
            return;
        }
        if kind == self.shape.block {
            self.visit_block(node);
            return;
        }
        self.count_fundamental(node);
        self.push_children(node);
    }

    fn count_fundamental(&mut self, node: Node<'tree>) {
        let kind = node.kind();
        let operators = self.profile.logical.cognitive_operators;
        if let Some(operator) = logical_operator(self.profile, node, self.source, operators) {
            self.boolean_operation(Some(operator));
        } else if self.is_negation(node) {
            self.boolean_operation(None);
        } else if self.profile.cognitive.labeled_jumps.contains(&kind) && node.named_child_count() > 0
            || kind == self.shape.invocation && self.is_recursive_call(node)
        {
            self.complexity = self.complexity.saturating_add(1);
        }
    }

    fn boolean_operation(&mut self, operator: Option<&'context str>) {
        if self.boolean_run != operator {
            if operator.is_some() {
                self.complexity = self.complexity.saturating_add(1);
            }
            self.boolean_run = operator;
        }
    }

    fn is_negation(&self, node: Node<'_>) -> bool {
        let (negation_kind, negation_operator) = self.shape.negation;
        node.kind() == negation_kind
            && node
                .child_by_field_name("operator")
                .and_then(|operator| operator.utf8_text(self.source).ok())
                .is_some_and(|operator| operator == negation_operator)
    }

    fn is_recursive_call(&self, call: Node<'_>) -> bool {
        let Some(recursion) = &self.recursion else {
            return false;
        };
        let same_name = call
            .child_by_field_name(NAME_FIELD)
            .and_then(|name| name.utf8_text(self.source).ok())
            .is_some_and(|name| name == recursion.name);
        let on_itself = call
            .child_by_field_name(OBJECT_FIELD)
            .is_none_or(|object| object.kind() == self.shape.this);
        let arity = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .map_or(0, |arguments| arguments.named_child_count());
        same_name && on_itself && arity == recursion.arity
    }

    fn visit_if(&mut self, node: Node<'tree>) {
        let if_kind = self.profile.cognitive.if_kind;
        let else_if = node.parent().is_some_and(|parent| parent.kind() == if_kind);
        let mut sequence = Vec::new();
        sequence.extend(node.child_by_field_name(CONDITION_FIELD).map(Event::Visit));
        if !else_if {
            sequence.push(Event::Structural);
        }
        sequence.extend(node.child_by_field_name(CONSEQUENCE_FIELD).map(Event::Visit));
        if !else_if {
            sequence.push(Event::DecreaseNesting);
        }
        if let Some(alternative) = node.child_by_field_name(ALTERNATIVE_FIELD) {
            sequence.extend([Event::Hybrid, Event::Visit(alternative), Event::DecreaseNesting]);
        }
        self.events.extend(sequence.into_iter().rev());
    }

    fn visit_block(&mut self, block: Node<'tree>) {
        for child in (0..block.child_count())
            .rev()
            .filter_map(|index| block.child(index))
        {
            self.events.push(Event::Visit(child));
            self.events.push(Event::EndBooleanRun);
        }
    }

    fn wrap_children(&mut self, node: Node<'tree>, opening: Event<'tree>) {
        self.events.push(Event::DecreaseNesting);
        self.push_children(node);
        self.events.push(opening);
    }

    fn push_children(&mut self, node: Node<'tree>) {
        let children = (0..node.child_count())
            .rev()
            .filter_map(|index| node.child(index));
        self.events.extend(children.map(Event::Visit));
    }
}
