use tree_sitter::Node;

use super::flow::{self, CaseBody};
use super::loops;
use super::reachability::Reachability;
use super::roles::{
    CODE_PATH_FUNCTIONS, LABELED, METHOD, NEW, Place, STATIC_BLOCK, SWITCH_CASES, SWITCH_DEFAULT,
    children_with_fields, first_argument, forks_by_result, is_chain_root, is_field_value, is_identifier,
    is_identifier_reference, is_optional, is_tagged_template, label_of, logical_kind, loop_kind,
    simple_constant,
};
use super::segments::Graph;
use super::shapes::{
    LoopPart, case_statements, has_case, is_first_case, is_pattern_with_default, labeled_body_is_breakable,
    preprocess_loop,
};
use super::state::{ChoiceKind, State};
use crate::lint::ecmascript::nodes::{CALL, COMMENT, MEMBER, PARENTHESIZED, PROGRAM, SUBSCRIPT, text};

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

type Children<'tree> = Vec<(Node<'tree>, Option<&'tree str>)>;

// El `CodePathAnalyzer` de ESLint sobre el árbol de tree-sitter. Lleva la pila de ancestros con el
// campo de cada uno porque `Node::parent` busca desde la raíz y es cuadrático en archivos grandes.
pub struct Analyzer<'tree, 'source> {
    source: &'source [u8],
    graph: Graph,
    paths: Vec<State>,
    reachability: Reachability,
    ancestors: Vec<(Node<'tree>, Option<&'tree str>)>,
}

impl<'tree, 'source> Analyzer<'tree, 'source> {
    pub fn analyze(root: Node<'tree>, source: &'source [u8]) -> Reachability {
        let mut analyzer = Self {
            source,
            graph: Graph::default(),
            paths: Vec::new(),
            reachability: Reachability::default(),
            ancestors: Vec::new(),
        };
        analyzer.visit(root, None);
        analyzer.reachability
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

    // El padre como lo ve ESTree, salteando paréntesis, y el campo que ocupa el nodo en él.
    fn estree_place(&self, field: Option<&'tree str>) -> Place<'tree> {
        let mut field = field;
        for (ancestor, own_field) in self.ancestors.iter().rev() {
            if ancestor.kind() != PARENTHESIZED {
                return Place {
                    parent: Some(*ancestor),
                    field,
                };
            }
            field = *own_field;
        }
        Place::default()
    }

    fn grandparent(&self) -> Option<Node<'tree>> {
        self.ancestors
            .len()
            .checked_sub(2)
            .and_then(|index| self.ancestors.get(index))
            .map(|(node, _)| *node)
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
            self.visit_method(node, field);
            return;
        }
        let place = self.place(field);
        let chain = is_chain_root(node, place);
        self.preprocess(node, place);
        if chain {
            if let Some((state, _)) = self.state() {
                state.push_chain();
            }
            self.forward();
        }
        self.enter(node, field);
        self.reachability.at_enter.insert(node.id(), self.reachable());
        self.visit_children(node, field, children_with_fields(node));
        self.exit(node, place);
        self.reachability.at_exit.insert(node.id(), self.reachable());
        self.postprocess(node, place);
        if chain {
            if let Some((state, graph)) = self.state() {
                state.pop_chain(graph);
            }
            self.forward();
        }
    }

    // Un método es dos nodos de ESTree: la clave va en el camino de afuera y la función en uno propio.
    fn visit_method(&mut self, method: Node<'tree>, field: Option<&'tree str>) {
        self.preprocess(method, self.place(field));
        self.forward();
        self.reachability.at_enter.insert(method.id(), self.reachable());
        let (key, function): (Children<'tree>, Children<'tree>) = children_with_fields(method)
            .into_iter()
            .partition(|(child, child_field)| child.kind() == DECORATOR || *child_field == Some(NAME_FIELD));
        self.visit_children(method, field, key);
        if function
            .iter()
            .any(|(_, child_field)| *child_field == Some(BODY_FIELD))
        {
            self.start_path();
            self.forward();
            self.visit_children(method, field, function);
            self.forward();
            self.end_path();
        } else {
            self.visit_children(method, field, function);
        }
        self.forward();
        self.reachability.at_exit.insert(method.id(), self.reachable());
    }

    fn start_path(&mut self) {
        if !self.paths.is_empty() {
            self.forward();
        }
        let state = State::new(&mut self.graph);
        self.paths.push(state);
    }

    fn end_path(&mut self) {
        if let Some(mut state) = self.paths.pop() {
            state.make_final(&self.graph);
        }
    }

    // `forwardCurrentToHead`: los segmentos actuales pasan a ser la cabeza del contexto.
    fn forward(&mut self) {
        let Some(state) = self.paths.last_mut() else {
            return;
        };
        let head = self.graph.head(state.fork);
        for (index, segment) in head.iter().enumerate() {
            if state.current.get(index) != Some(segment) {
                self.graph.mark_used(*segment);
            }
        }
        state.current = head;
    }

    fn preprocess(&mut self, node: Node<'tree>, place: Place<'tree>) {
        let Some(parent) = place.parent else {
            return;
        };
        let source = self.source;
        let grandparent = self.grandparent();
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
        let estree_place = self.estree_place(field);
        if is_field_value(place) {
            self.start_path();
        }
        if kind == PROGRAM || kind == STATIC_BLOCK || CODE_PATH_FUNCTIONS.contains(&kind) {
            self.start_path();
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
