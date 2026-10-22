use std::collections::HashMap;

use tree_sitter::Node;

use super::calls::sink_at;
use super::imports::Imports;
use super::parameter_patterns;
use crate::lint::taint::folding::{same_label, truth};
use crate::lint::taint::responses::lambda_html_body;
use crate::lint::taint::{Flow, SinkKind, SinkReceiver, State, TaintCall, TaintSink, Value, join_states};
use crate::syntax::named_children;

const BLOCK: &str = "statement_block";
const DANGEROUS_HTML: &str = "dangerouslySetInnerHTML";
const RETURN_SUBJECT: &str = "return";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const DECLARATIONS: &[&str] = &["lexical_declaration", "variable_declaration"];
const DECLARATOR: &str = "variable_declarator";
const IF: &str = "if_statement";
const ELSE: &str = "else_clause";
const LOOPS: &[&str] = &[
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
];
const FOR_IN: &str = "for_in_statement";
const TRY: &str = "try_statement";
const SWITCH: &str = "switch_statement";
const SWITCH_CASE: &str = "switch_case";
const SWITCH_DEFAULT: &str = "switch_default";
const RETURN: &str = "return_statement";
const THROW: &str = "throw_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const COMMENT: &str = "comment";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const NAME_FIELD: &str = "name";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const HANDLER_FIELD: &str = "handler";
const FINALIZER_FIELD: &str = "finalizer";
const PARAMETER_FIELD: &str = "parameter";
const INITIALIZER_FIELD: &str = "initializer";
const INCREMENT_FIELD: &str = "increment";
// Dos vueltas alcanzan para que lo que entra en una vuelta llegue a la siguiente.
const LOOP_PASSES: usize = 2;

// Recorre una función con lo que se sabe de cada variable. Los caminos que salen (`return`,
// `throw`) no siguen; un `break` o un `continue` terminan el bucle o el `switch` que los contiene.
pub struct Interpreter<'source, 'context> {
    pub(super) source: &'source [u8],
    pub(super) imports: &'context Imports,
    pub(super) locals: &'context HashMap<String, String>,
    pub(super) returns: Flow,
    pub(super) calls: Vec<TaintCall>,
    pub(super) sinks: Vec<TaintSink>,
    pub(super) plain_response: bool,
    pub(super) lambda: bool,
    exits: Vec<Option<State>>,
}

impl<'source, 'context> Interpreter<'source, 'context> {
    pub fn new(
        source: &'source [u8],
        imports: &'context Imports,
        locals: &'context HashMap<String, String>,
    ) -> Self {
        Self {
            source,
            imports,
            locals,
            returns: Flow::default(),
            calls: Vec::new(),
            sinks: Vec::new(),
            plain_response: false,
            lambda: false,
            exits: Vec::new(),
        }
    }

    pub(super) fn text(&self, node: Node<'_>) -> &'source str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    // Una función de flecha con una expresión como cuerpo devuelve esa expresión. Si la función
    // declara que la respuesta no es HTML, sus escrituras no son un XSS.
    pub fn run(&mut self, body: Node<'_>, state: State) {
        self.walk_body(body, state);
        self.drop_html_sinks_from(0);
    }

    fn walk_body(&mut self, body: Node<'_>, mut state: State) -> Option<State> {
        if body.kind() == BLOCK {
            return self.statement(body, state);
        }
        let returned = self.expression(body, &mut state).flow();
        self.returns.absorb(&returned);
        Some(state)
    }

    fn drop_html_sinks_from(&mut self, first: usize) {
        if !self.plain_response {
            return;
        }
        let mut position = 0;
        self.sinks.retain(|sink| {
            position += 1;
            position <= first
                || !matches!(sink.receiver, SinkReceiver::Known(SinkKind::Html))
                || sink.callee == DANGEROUS_HTML
        });
    }

    // Una función escrita dentro de otra (un callback de `.then`, `.map` o de una ruta) corre en el
    // lugar con lo que ve de afuera; sus parámetros valen `arguments`. Lo que les asigna a las
    // variables de afuera puede haber pasado o no, y lo que devuelve es su valor.
    pub(super) fn closure(&mut self, function: Node<'_>, state: &mut State, arguments: &Value) -> Value {
        let Some(body) = function.child_by_field_name(BODY_FIELD) else {
            return Value::clean();
        };
        let mut parameters = State::new();
        for parameter in parameter_patterns(function) {
            self.bind(parameter, arguments, &mut parameters);
        }
        let mut inner = state.clone();
        inner.extend(
            parameters
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        let outer_returns = std::mem::take(&mut self.returns);
        let outer_exits = std::mem::take(&mut self.exits);
        let outer_lambda = std::mem::replace(&mut self.lambda, false);
        let outer_plain = std::mem::replace(&mut self.plain_response, false);
        let first_sink = self.sinks.len();
        let after = self.walk_body(body, inner);
        self.drop_html_sinks_from(first_sink);
        let returned = std::mem::replace(&mut self.returns, outer_returns);
        self.exits = outer_exits;
        self.lambda = outer_lambda;
        self.plain_response = outer_plain;
        for (name, value) in after.into_iter().flatten() {
            if parameters.contains_key(&name) {
                continue;
            }
            if let Some(existing) = state.remove(&name) {
                state.insert(name, existing.join(value));
            }
        }
        Value::Data(returned)
    }

    fn statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        match node.kind() {
            BLOCK => self.sequence(node, state),
            EXPRESSION_STATEMENT => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                Some(state)
            }
            kind if DECLARATIONS.contains(&kind) => {
                for declarator in named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == DECLARATOR)
                {
                    self.declare(declarator, &mut state);
                }
                Some(state)
            }
            IF => self.if_statement(node, state),
            kind if LOOPS.contains(&kind) => self.repeat(node, state),
            TRY => self.try_statement(node, state),
            SWITCH => self.switch(node, state),
            RETURN => {
                self.return_statement(node, &mut state);
                None
            }
            THROW => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                None
            }
            BREAK | CONTINUE => {
                if let Some(exit) = self.exits.last_mut() {
                    *exit = join_states(exit.take(), Some(state));
                }
                None
            }
            _ => Some(state),
        }
    }

    // Lo que se devuelve; en un handler de Lambda, una respuesta HTML es un sumidero.
    fn return_statement(&mut self, node: Node<'_>, state: &mut State) {
        for child in named_children(node) {
            let returned = self.expression(child, state);
            if let Some(body) = self.lambda.then(|| lambda_html_body(&returned)).flatten() {
                self.sinks
                    .push(sink_at(node, RETURN_SUBJECT, SinkKind::Html, vec![body]));
            }
            self.returns.absorb(&returned.flow());
        }
    }

    fn sequence(&mut self, node: Node<'_>, state: State) -> Option<State> {
        named_children(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .try_fold(state, |current, child| self.statement(child, current))
    }

    fn declare(&mut self, declarator: Node<'_>, state: &mut State) {
        let (Some(target), Some(value)) = (
            declarator.child_by_field_name(NAME_FIELD),
            declarator.child_by_field_name(VALUE_FIELD),
        ) else {
            return;
        };
        let assigned = self.expression(value, state);
        self.bind(target, &assigned, state);
    }

    fn branch(&mut self, branch: Option<Node<'_>>, state: State) -> Option<State> {
        let Some(node) = branch else {
            return Some(state);
        };
        let body = if node.kind() == ELSE {
            named_children(node).into_iter().next()
        } else {
            Some(node)
        };
        match body {
            Some(inner) => self.statement(inner, state),
            None => Some(state),
        }
    }

    fn if_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let condition = node.child_by_field_name(CONDITION_FIELD)?;
        let decided = truth(self.expression(condition, &mut state).literal());
        let taken = (decided != Some(false))
            .then(|| self.branch(node.child_by_field_name(CONSEQUENCE_FIELD), state.clone()))
            .flatten();
        let other = (decided != Some(true))
            .then(|| self.branch(node.child_by_field_name(ALTERNATIVE_FIELD), state))
            .flatten();
        join_states(taken, other)
    }

    fn repeat(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        if let Some(initializer) = node.child_by_field_name(INITIALIZER_FIELD)
            && let Some(after) = self.statement(initializer, state.clone())
        {
            state = after;
        }
        if node.kind() == FOR_IN
            && let (Some(target), Some(iterable)) = (
                node.child_by_field_name(LEFT_FIELD),
                node.child_by_field_name(RIGHT_FIELD),
            )
        {
            let items = self.expression(iterable, &mut state).flow();
            self.bind(target, &Value::Data(items), &mut state);
        }
        self.exits.push(None);
        let mut entry = Some(state);
        for _ in 0..LOOP_PASSES {
            let Some(mut current) = entry.clone() else {
                break;
            };
            if let Some(condition) = node.child_by_field_name(CONDITION_FIELD) {
                self.expression(condition, &mut current);
            }
            let mut after = node
                .child_by_field_name(BODY_FIELD)
                .and_then(|body| self.statement(body, current));
            if let (Some(increment), Some(updated)) =
                (node.child_by_field_name(INCREMENT_FIELD), after.as_mut())
            {
                self.expression(increment, updated);
            }
            entry = join_states(entry, after);
        }
        let broken = self.exits.pop().flatten();
        join_states(entry, broken)
    }

    fn try_statement(&mut self, node: Node<'_>, state: State) -> Option<State> {
        let mut result = node
            .child_by_field_name(BODY_FIELD)
            .and_then(|body| self.statement(body, state.clone()));
        if let Some(handler) = node.child_by_field_name(HANDLER_FIELD) {
            let mut entry = join_states(Some(state.clone()), result.clone()).unwrap_or_default();
            if let Some(parameter) = handler.child_by_field_name(PARAMETER_FIELD) {
                self.bind(parameter, &Value::clean(), &mut entry);
            }
            let handled = handler
                .child_by_field_name(BODY_FIELD)
                .and_then(|body| self.statement(body, entry));
            result = join_states(result, handled);
        }
        let Some(finalizer) = node.child_by_field_name(FINALIZER_FIELD) else {
            return result;
        };
        let entry = join_states(Some(state), result.clone()).unwrap_or_default();
        let block = named_children(finalizer).into_iter().next();
        self.branch(block, entry).and(result)
    }

    // Con el valor del `switch` constante, sólo corre el caso que toca (y los que siguen hasta un
    // `break`); si no, todos.
    fn switch(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let subject = node
            .child_by_field_name(VALUE_FIELD)
            .map(|written| self.expression(written, &mut state))
            .and_then(|value| value.literal().cloned());
        let cases: Vec<Node<'_>> = node
            .child_by_field_name(BODY_FIELD)
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|case| case.kind() == SWITCH_CASE || case.kind() == SWITCH_DEFAULT)
            .collect();
        let matched = subject.as_ref().and_then(|value| {
            cases
                .iter()
                .position(|case| {
                    case.child_by_field_name(VALUE_FIELD)
                        .and_then(|label| super::expressions::literal_of(label, self.source))
                        .is_some_and(|label| same_label(value, &label))
                })
                .or_else(|| cases.iter().position(|case| case.kind() == SWITCH_DEFAULT))
        });
        self.exits.push(None);
        let mut falling: Option<State> = None;
        for (position, case) in cases.iter().enumerate() {
            let enters = subject.is_none() || matched == Some(position);
            let Some(entry) = join_states(falling.take(), enters.then(|| state.clone())) else {
                continue;
            };
            let label = case.child_by_field_name(VALUE_FIELD);
            falling = named_children(*case)
                .into_iter()
                .filter(|child| Some(*child) != label)
                .try_fold(entry, |current, child| self.statement(child, current));
        }
        let broken = self.exits.pop().flatten();
        let skipped = (subject.is_none() || matched.is_none()).then_some(state);
        join_states(join_states(falling, broken), skipped)
    }
}
