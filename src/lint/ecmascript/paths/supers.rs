use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::segments::{Graph, SegmentId};
use super::traversal::traverse_segments;
use crate::lint::ecmascript::nodes::text;

const METHOD: &str = "method_definition";
const CLASS_BODY: &str = "class_body";
const CLASS_HERITAGE: &str = "class_heritage";
const EXTENDS_CLAUSE: &str = "extends_clause";
const NAME_FIELD: &str = "name";
const VALUE_FIELD: &str = "value";
const CONSTRUCTOR: &str = "constructor";
const NOT_CONSTRUCTOR_KINDS: [&str; 3] = ["static", "get", "set"];
const QUOTES: [char; 2] = ['"', '\''];
const PARENTHESIZED: &str = "parenthesized_expression";
const OPERATOR_FIELD: &str = "operator";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const UNDEFINED: &str = "undefined";
const NULL: &str = "null";
const VOID: &str = "void";
const IDENTIFIER: &str = "identifier";
const ASSIGNMENT: &str = "assignment_expression";
const AUGMENTED_ASSIGNMENT: &str = "augmented_assignment_expression";
const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
const SEQUENCE: &str = "sequence_expression";
const UNARY: &str = "unary_expression";
const AND: &str = "&&";
const OR: &str = "||";
const NULLISH: &str = "??";
const AND_ASSIGN: &str = "&&=";
const OR_ASSIGN: &str = "||=";
const NULLISH_ASSIGN: &str = "??=";
const ALWAYS_CONSTRUCTORS: [&str; 10] = [
    "class",
    "function_expression",
    "function",
    "this",
    "member_expression",
    "subscript_expression",
    "call_expression",
    "new_expression",
    "yield_expression",
    "meta_property",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperProblem {
    MissingSome,
    MissingAll,
    Duplicate,
    NotConstructor,
    BeforeSuper,
}

#[derive(Debug, Clone, Copy)]
pub struct SuperReport<'tree> {
    pub problem: SuperProblem,
    pub node: Node<'tree>,
}

#[derive(Debug, Default)]
struct Calls<'tree> {
    some: bool,
    every: bool,
    valid: Vec<Node<'tree>>,
}

#[derive(Debug, Default)]
struct Before<'tree> {
    called: bool,
    invalid: Vec<Node<'tree>>,
}

// Lo que `constructor-super` y `no-this-before-super` llevan del constructor de una clase con
// `extends`, segmento por segmento.
#[derive(Debug)]
pub struct DerivedConstructor<'tree> {
    method: Node<'tree>,
    super_is_constructor: bool,
    checks_this: bool,
    calls: HashMap<SegmentId, Calls<'tree>>,
    before: HashMap<SegmentId, Before<'tree>>,
    reports: Vec<SuperReport<'tree>>,
}

impl<'tree> DerivedConstructor<'tree> {
    // `isConstructorFunction` con la clase que tiene `extends`.
    pub fn of(start: Node<'tree>, source: &[u8]) -> Option<Self> {
        if start.kind() != METHOD || !is_constructor_method(start, source) {
            return None;
        }
        let class = start
            .parent()
            .filter(|body| body.kind() == CLASS_BODY)?
            .parent()?;
        let super_class = super_class(class)?;
        Some(Self {
            method: start,
            super_is_constructor: is_possible_constructor(super_class, source),
            checks_this: !is_null_or_undefined(super_class, source),
            calls: HashMap::new(),
            before: HashMap::new(),
            reports: Vec::new(),
        })
    }

    pub fn segment_start(&mut self, segment: SegmentId, graph: &Graph, for_update: bool) {
        let previous = graph.previous(segment);
        let seen: Vec<SegmentId> = previous
            .iter()
            .copied()
            .filter(|prior| self.calls.contains_key(prior))
            .collect();
        let mut calls = Calls::default();
        if !seen.is_empty() {
            calls.some = seen.iter().any(|prior| self.called_in_some(graph, *prior));
            calls.every = seen.iter().all(|prior| self.called_in_every(graph, *prior));
        }
        if for_update {
            calls.every = true;
        }
        self.calls.insert(segment, calls);
        let called = !previous.is_empty() && previous.iter().all(|prior| self.is_called(graph, *prior));
        self.before.insert(
            segment,
            Before {
                called,
                invalid: Vec::new(),
            },
        );
    }

    pub fn segment_loop(&mut self, from: SegmentId, to: SegmentId, graph: &Graph) {
        traverse_segments(graph, to, Some(from), |segment| {
            self.carry_calls_around_loop(segment, graph)
        });
        traverse_segments(graph, to, Some(from), |segment| {
            let previous = graph.previous(segment);
            let called_before =
                !previous.is_empty() && previous.iter().all(|prior| self.is_called(graph, *prior));
            let before = self.before.entry(segment).or_default();
            if before.called {
                return true;
            }
            if called_before {
                before.called = true;
            }
            false
        });
    }

    // Una vuelta de `constructor-super` por el loop: lo llamado antes pasa a los segmentos de adentro
    // y las llamadas que ya eran válidas se vuelven duplicadas.
    fn carry_calls_around_loop(&mut self, segment: SegmentId, graph: &Graph) -> bool {
        if !self.calls.contains_key(&segment) {
            return true;
        }
        let seen: Vec<SegmentId> = graph
            .previous(segment)
            .iter()
            .copied()
            .filter(|prior| self.calls.contains_key(prior))
            .collect();
        let some_before = seen.iter().any(|prior| self.called_in_some(graph, *prior));
        let every_before = seen.iter().all(|prior| self.called_in_every(graph, *prior));
        let Some(calls) = self.calls.get_mut(&segment) else {
            return false;
        };
        calls.some |= some_before;
        calls.every |= every_before;
        let duplicates = if some_before {
            std::mem::take(&mut calls.valid)
        } else {
            Vec::new()
        };
        self.reports
            .extend(duplicates.into_iter().map(|node| SuperReport {
                problem: SuperProblem::Duplicate,
                node,
            }));
        false
    }

    pub fn super_call(&mut self, call: Node<'tree>, current: &[SegmentId], graph: &Graph) {
        let checks_this_now = self.is_before_super(current, graph);
        let mut duplicate = false;
        let mut last = None;
        for segment in current {
            if let Some(calls) = self.calls.get_mut(segment) {
                duplicate |= calls.some;
                calls.some = true;
                calls.every = true;
                last = Some(*segment);
            }
        }
        if let Some(segment) = last {
            if duplicate {
                self.report(SuperProblem::Duplicate, call);
            } else if !self.super_is_constructor {
                self.report(SuperProblem::NotConstructor, call);
            } else if let Some(calls) = self.calls.get_mut(&segment) {
                calls.valid.push(call);
            }
        }
        if checks_this_now {
            for segment in current {
                self.before.entry(*segment).or_default().called = true;
            }
        }
    }

    pub fn this_or_super(&mut self, node: Node<'tree>, current: &[SegmentId], graph: &Graph) {
        if self.is_before_super(current, graph) {
            for segment in current {
                self.before.entry(*segment).or_default().invalid.push(node);
            }
        }
    }

    pub fn return_with_value(&mut self, current: &[SegmentId]) {
        for segment in current {
            if let Some(calls) = self.calls.get_mut(segment) {
                calls.some = true;
                calls.every = true;
            }
        }
    }

    pub fn finish(&mut self, returned: &[SegmentId], initial: Option<SegmentId>, graph: &Graph) {
        let every = returned
            .iter()
            .all(|segment| self.called_in_every(graph, *segment));
        let some = returned
            .iter()
            .any(|segment| self.called_in_some(graph, *segment));
        if !every {
            let problem = if some {
                SuperProblem::MissingSome
            } else {
                SuperProblem::MissingAll
            };
            self.report(problem, self.method);
        }
        let (Some(initial), true) = (initial, self.checks_this) else {
            return;
        };
        let mut reported: HashSet<usize> = HashSet::new();
        let mut found = Vec::new();
        traverse_segments(graph, initial, None, |segment| {
            let Some(before) = self.before.get(&segment) else {
                return false;
            };
            found.extend(
                before
                    .invalid
                    .iter()
                    .filter(|node| reported.insert(node.id()))
                    .copied(),
            );
            before.called
        });
        self.reports.extend(found.into_iter().map(|node| SuperReport {
            problem: SuperProblem::BeforeSuper,
            node,
        }));
    }

    pub fn reports(&self) -> &[SuperReport<'tree>] {
        &self.reports
    }

    fn report(&mut self, problem: SuperProblem, node: Node<'tree>) {
        self.reports.push(SuperReport { problem, node });
    }

    fn called_in_some(&self, graph: &Graph, segment: SegmentId) -> bool {
        graph.is_reachable(segment) && self.calls.get(&segment).is_some_and(|calls| calls.some)
    }

    fn called_in_every(&self, graph: &Graph, segment: SegmentId) -> bool {
        graph.is_reachable(segment) && self.calls.get(&segment).is_some_and(|calls| calls.every)
    }

    fn is_called(&self, graph: &Graph, segment: SegmentId) -> bool {
        !graph.is_reachable(segment) || self.before.get(&segment).is_some_and(|before| before.called)
    }

    fn is_before_super(&self, current: &[SegmentId], graph: &Graph) -> bool {
        self.checks_this && !current.iter().all(|segment| self.is_called(graph, *segment))
    }
}

// Un método `constructor` que no es estático, getter ni setter (el `kind: "constructor"` de ESTree).
fn is_constructor_method(method: Node<'_>, source: &[u8]) -> bool {
    let Some(name) = method.child_by_field_name(NAME_FIELD) else {
        return false;
    };
    let mut cursor = method.walk();
    let modified = method
        .children(&mut cursor)
        .take_while(|child| child.id() != name.id())
        .any(|child| NOT_CONSTRUCTOR_KINDS.contains(&child.kind()));
    !modified && text(name, source).trim_matches(QUOTES) == CONSTRUCTOR
}

fn super_class(class: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = class.walk();
    let heritage = class
        .named_children(&mut cursor)
        .find(|child| child.kind() == CLASS_HERITAGE)?;
    let first = heritage.named_child(0)?;
    if first.kind() == EXTENDS_CLAUSE {
        return first.child_by_field_name(VALUE_FIELD);
    }
    Some(first)
}

fn unparenthesized(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == PARENTHESIZED {
        match node.named_child(0) {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

// `isPossibleConstructor` de `constructor-super`, sobre los nodos de tree-sitter.
fn is_possible_constructor(node: Node<'_>, source: &[u8]) -> bool {
    let node = unparenthesized(node);
    let field = |name: &str| node.child_by_field_name(name);
    let possible =
        |child: Option<Node<'_>>| child.is_some_and(|child| is_possible_constructor(child, source));
    let operator = field(OPERATOR_FIELD).map(|operator| text(operator, source));
    match node.kind() {
        kind if ALWAYS_CONSTRUCTORS.contains(&kind) => true,
        IDENTIFIER => text(node, source) != UNDEFINED,
        ASSIGNMENT => possible(field(RIGHT_FIELD)),
        AUGMENTED_ASSIGNMENT => match operator {
            Some(AND_ASSIGN) => possible(field(RIGHT_FIELD)),
            Some(OR_ASSIGN | NULLISH_ASSIGN) => possible(field(LEFT_FIELD)) || possible(field(RIGHT_FIELD)),
            _ => false,
        },
        BINARY => match operator {
            Some(AND) => possible(field(RIGHT_FIELD)),
            Some(OR | NULLISH) => possible(field(LEFT_FIELD)) || possible(field(RIGHT_FIELD)),
            _ => false,
        },
        TERNARY => possible(field(ALTERNATIVE_FIELD)) || possible(field(CONSEQUENCE_FIELD)),
        SEQUENCE => possible(node.named_children(&mut node.walk()).last()),
        _ => false,
    }
}

// `isNullOrUndefined` de las utilidades de ESLint: `null`, `undefined` o `void x`.
fn is_null_or_undefined(node: Node<'_>, source: &[u8]) -> bool {
    let node = unparenthesized(node);
    match node.kind() {
        NULL | UNDEFINED => true,
        IDENTIFIER => text(node, source) == UNDEFINED,
        UNARY => node
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == VOID),
        _ => false,
    }
}
