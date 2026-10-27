use tree_sitter::Node;

use super::flow::{self, CaseBody};
use super::loops;
use super::reachability::{Flow, PathRecord, segment_changes};
use super::roles::{
    CODE_PATH_FUNCTIONS, LABELED, METHOD, NEW, Place, STATIC_BLOCK, SWITCH_CASES, SWITCH_DEFAULT,
    children_with_fields, first_argument, forks_by_result, is_chain_root, is_field_value, is_identifier,
    is_identifier_reference, is_optional, is_tagged_template, label_of, logical_kind, loop_kind,
    simple_constant,
};
use super::segments::Graph;
use super::shapes::{
    LoopPart, case_statements, estree_place, has_case, is_first_case, is_pattern_with_default,
    labeled_body_is_breakable, preprocess_loop,
};
use super::state::{ChoiceKind, State};
use super::supers::DerivedConstructor;
use crate::lint::ecmascript::nodes::{CALL, COMMENT, MEMBER, PROGRAM, SUBSCRIPT, text};
use super_events::is_for_update;

const IF: &str = "if_statement";
const TERNARY: &str = "ternary_expression";
const ELSE: &str = "else_clause";
const SWITCH: &str = "switch_statement";
const TRY: &str = "try_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const RETURN: &str = "return_statement";
const THROW: &str = "throw_statement";
const YIELD: &str = "yield_expression";
const ARGUMENTS: &str = "arguments";
const DECORATOR: &str = "decorator";
const EMPTY_STATEMENT: &str = "empty_statement";
const LABEL_FIELD: &str = "label";
const BODY_FIELD: &str = "body";
const NAME_FIELD: &str = "name";
const RIGHT_FIELD: &str = "right";
const PROPERTY_FIELD: &str = "property";
const INDEX_FIELD: &str = "index";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const HANDLER_FIELD: &str = "handler";
const FINALIZER_FIELD: &str = "finalizer";

mod methods;
mod super_events;

type Children<'tree> = Vec<(Node<'tree>, Option<&'tree str>)>;

// El `CodePathAnalyzer` de ESLint sobre el árbol de tree-sitter. Lleva la pila de ancestros con el
// campo de cada uno porque `Node::parent` busca desde la raíz y es cuadrático en archivos grandes.
pub struct Analyzer<'tree, 'source> {
    source: &'source [u8],
    graph: Graph,
    paths: Vec<State>,
    records: Vec<PathRecord<'tree>>,
    flow: Flow<'tree>,
    ancestors: Vec<(Node<'tree>, Option<&'tree str>)>,
    in_for_update: bool,
}

impl<'tree, 'source> Analyzer<'tree, 'source> {
    pub fn analyze(root: Node<'tree>, source: &'source [u8]) -> Flow<'tree> {
        let mut analyzer = Self {
            source,
            graph: Graph::default(),
            paths: Vec::new(),
            records: Vec::new(),
            flow: Flow::default(),
            ancestors: Vec::new(),
            in_for_update: false,
        };
        analyzer.visit(root, None);
        analyzer.flow.successors = analyzer.graph.successor_lists();
        analyzer.flow
    }

    fn state(&mut self) -> Option<(&mut State, &mut Graph)> {
        self.paths.last_mut().map(|state| (state, &mut self.graph))
    }

    fn reachable(&self) -> bool {
        self.paths
            .last()
            .is_some_and(|state| self.graph.any_reachable(&state.current))
    }

    fn place(&self, field: Option<&'tree str>) -> Place<'tree> {
        Place {
            parent: self.ancestors.last().map(|(parent, _)| *parent),
            field,
        }
    }

    fn visit_children(&mut self, node: Node<'tree>, field: Option<&'tree str>, children: Children<'tree>) {
        self.ancestors.push((node, field));
        for (child, child_field) in children {
            self.visit(child, child_field);
        }
        self.ancestors.pop();
    }

    fn visit(&mut self, node: Node<'tree>, field: Option<&'tree str>) {
        if node.kind() == COMMENT {
            return;
        }
        if node.kind() == METHOD {
            methods::visit_method(self, node, field);
            return;
        }
        let place = self.place(field);
        let chain = is_chain_root(node, place);
        self.in_for_update = is_for_update(place);
        self.preprocess(node, place);
        if chain {
            if let Some((state, _)) = self.state() {
                state.push_chain();
            }
            self.forward();
        }
        self.enter(node, field);
        self.flow.at_enter.insert(node.id(), self.reachable());
        if let Some(record) = self.records.last_mut() {
            record.observe_enter(node);
        }
        super_events::observe_super_enter(self, node, place);
        self.visit_children(node, field, children_with_fields(node));
        self.in_for_update = is_for_update(place);
        self.exit(node, place);
        self.flow.at_exit.insert(node.id(), self.reachable());
        if let Some(record) = self.records.last_mut() {
            record.observe_exit(node);
        }
        super_events::observe_super_call(self, node);
        self.postprocess(node, place);
        if chain {
            if let Some((state, graph)) = self.state() {
                state.pop_chain(graph);
            }
            self.forward();
        }
    }

    fn start_path(&mut self, start: Node<'tree>) {
        if !self.paths.is_empty() {
            self.forward();
        }
        let state = State::new(&mut self.graph);
        let mut record = PathRecord::new(start);
        record.initial = self.graph.head(state.fork).first().copied();
        record.derived = DerivedConstructor::of(start, self.source);
        self.paths.push(state);
        self.records.push(record);
    }

    fn end_path(&mut self) {
        super_events::notify_loops(self);
        let returned = self.paths.pop().map(|mut state| {
            state.make_final(&self.graph);
            state.returned
        });
        if let Some(mut record) = self.records.pop() {
            let initial = record.initial;
            if let Some(derived) = record.derived.as_mut() {
                derived.finish(&returned.unwrap_or_default(), initial, &self.graph);
            }
            self.flow.paths.push(record);
        }
    }

    // `forwardCurrentToHead`: los segmentos actuales pasan a ser la cabeza del contexto.
    fn forward(&mut self) {
        super_events::notify_loops(self);
        let Some(state) = self.paths.last_mut() else {
            return;
        };
        let head = self.graph.head(state.fork);
        let (ended, started) = segment_changes(&state.current, &head);
        for segment in &started {
            self.graph.mark_used(*segment);
        }
        state.current = head;
        let Some(record) = self.records.last_mut() else {
            return;
        };
        for segment in ended {
            record.end_segment(segment);
        }
        for segment in started
            .into_iter()
            .filter(|segment| self.graph.is_reachable(*segment))
        {
            record.start_segment(segment);
            if let Some(derived) = record.derived.as_mut() {
                derived.segment_start(segment, &self.graph, self.in_for_update);
            }
        }
    }

    fn preprocess(&mut self, node: Node<'tree>, place: Place<'tree>) {
        let Some(parent) = place.parent else {
            return;
        };
        let source = self.source;
        let grandparent = self.ancestors.iter().rev().nth(1).map(|(ancestor, _)| *ancestor);
        let Some((state, graph)) = self.state() else {
            return;
        };
        let kind = parent.kind();
        let is = |field: &str| place.is(field);
        let is_optional_argument = kind == ARGUMENTS
            && grandparent.is_some_and(|call| {
                call.kind() == CALL
                    && is_optional(call)
                    && first_argument(call).is_some_and(|first| first.id() == node.id())
            });
        let is_optional_property = (kind == MEMBER && is(PROPERTY_FIELD)
            || kind == SUBSCRIPT && is(INDEX_FIELD))
            && is_optional(parent);
        if is_optional_argument || is_optional_property {
            state.make_optional_right(graph);
        } else if logical_kind(parent, source).is_some() && is(RIGHT_FIELD) {
            state.make_logical_right(graph);
        } else if (kind == IF || kind == TERNARY) && is(CONSEQUENCE_FIELD) {
            state.make_if_consequent(graph);
        } else if (kind == IF && node.kind() == ELSE) || (kind == TERNARY && is(ALTERNATIVE_FIELD)) {
            state.make_if_alternate(graph);
        } else if SWITCH_CASES.contains(&kind)
            && case_statements(parent)
                .first()
                .is_some_and(|first| first.id() == node.id())
        {
            flow::make_switch_case_body(
                state,
                graph,
                CaseBody {
                    empty: false,
                    default: kind == SWITCH_DEFAULT,
                },
            );
        } else if kind == TRY && is(HANDLER_FIELD) {
            flow::make_catch_block(state, graph);
        } else if kind == TRY && is(FINALIZER_FIELD) {
            flow::make_finally_block(state, graph);
        } else {
            let part = if place.is(CONDITION_FIELD) && node.kind() != EMPTY_STATEMENT {
                LoopPart::Test(simple_constant(node, source))
            } else {
                LoopPart::Other
            };
            preprocess_loop(state, graph, place, part);
        }
    }

    fn enter(&mut self, node: Node<'tree>, field: Option<&'tree str>) {
        let source = self.source;
        let kind = node.kind();
        let place = self.place(field);
        let estree_place = estree_place(&self.ancestors, field);
        if is_field_value(place) {
            self.start_path(node);
        }
        if kind == PROGRAM || kind == STATIC_BLOCK || CODE_PATH_FUNCTIONS.contains(&kind) {
            self.start_path(node);
        }
        if let Some((state, graph)) = self.state() {
            if let Some(choice) = logical_kind(node, source) {
                state.push_choice(graph, choice, forks_by_result(estree_place, source));
            } else if (kind == CALL && !is_tagged_template(node) || kind == MEMBER || kind == SUBSCRIPT)
                && is_optional(node)
            {
                state.make_optional_node(graph);
            } else if kind == IF || kind == TERNARY {
                state.push_choice(graph, ChoiceKind::Test, false);
            } else if kind == SWITCH {
                flow::push_switch(state, graph, has_case(node), label_of(place, source));
            } else if kind == TRY {
                flow::push_try(state, graph, node.child_by_field_name(FINALIZER_FIELD).is_some());
            } else if SWITCH_CASES.contains(&kind)
                && !place.parent.is_some_and(|body| is_first_case(body, node))
            {
                state.fork_path(graph);
            } else if let Some(loop_kind) = loop_kind(kind) {
                loops::push_loop(state, graph, loop_kind, label_of(place, source));
            } else if kind == LABELED && !labeled_body_is_breakable(node) {
                let label = node
                    .child_by_field_name(LABEL_FIELD)
                    .map(|label| text(label, source).to_owned());
                state.push_break(graph, false, label);
            }
        }
        self.forward();
    }

    fn exit(&mut self, node: Node<'tree>, place: Place<'tree>) {
        let source = self.source;
        let kind = node.kind();
        let label = node
            .child_by_field_name(LABEL_FIELD)
            .map(|label| text(label, source).to_owned());
        if [BREAK, CONTINUE, RETURN, THROW].contains(&kind) {
            self.forward();
        }
        let Some((state, graph)) = self.state() else {
            return;
        };
        let mut forward = true;
        if kind == IF || kind == TERNARY || logical_kind(node, source).is_some() {
            state.pop_choice(graph);
        } else if kind == SWITCH {
            flow::pop_switch(state, graph);
        } else if SWITCH_CASES.contains(&kind) {
            if case_statements(node).is_empty() {
                flow::make_switch_case_body(
                    state,
                    graph,
                    CaseBody {
                        empty: true,
                        default: kind == SWITCH_DEFAULT,
                    },
                );
            }
            forward = !graph.is_fork_reachable(state.fork);
        } else if kind == TRY {
            flow::pop_try(state, graph);
        } else if kind == BREAK {
            state.make_break(graph, label.as_deref());
            forward = false;
        } else if kind == CONTINUE {
            state.make_continue(graph, label.as_deref());
            forward = false;
        } else if kind == RETURN {
            state.make_return(graph);
            forward = false;
        } else if kind == THROW {
            state.make_throw(graph);
            forward = false;
        } else if is_identifier(kind) {
            if is_identifier_reference(node, place) {
                state.make_first_throwable(graph);
                forward = false;
            }
        } else if (kind == CALL && !is_tagged_template(node))
            || kind == MEMBER
            || kind == SUBSCRIPT
            || kind == NEW
        {
            state.make_first_throwable(graph);
        } else if kind == YIELD {
            state.make_yield(graph);
        } else if loop_kind(kind).is_some() {
            loops::pop_loop(state, graph);
        } else if is_pattern_with_default(node) {
            state.pop_fork(graph);
        } else if kind == LABELED && !labeled_body_is_breakable(node) {
            state.pop_break(graph);
        }
        if forward {
            self.forward();
        }
    }

    fn postprocess(&mut self, node: Node<'tree>, place: Place<'tree>) {
        let kind = node.kind();
        if kind == PROGRAM || kind == STATIC_BLOCK || CODE_PATH_FUNCTIONS.contains(&kind) {
            self.end_path();
        } else if kind == CALL
            && is_optional(node)
            && first_argument(node).is_none()
            && let Some((state, graph)) = self.state()
        {
            state.make_optional_right(graph);
        }
        if is_field_value(place) {
            self.end_path();
        }
    }
}

// `test` es `Some` cuando el nodo es la condición del loop, con su valor si es un literal.
