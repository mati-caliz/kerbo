use tree_sitter::Node;

use super::calls::sink_at;
use super::expressions::literal_of;
use super::imports::Imports;
use super::types::Types;
use crate::lint::taint::folding::{same_label, truth};
use crate::lint::taint::responses::lambda_html_body;
use crate::lint::taint::{Flow, SinkKind, SinkReceiver, State, TaintCall, TaintSink, Value, join_states};
use crate::syntax::named_children;

const BLOCK: &str = "block";
const STATEMENT_LIST: &str = "statement_list";
const EXPRESSION_STATEMENTS: &[&str] = &["expression_statement", "go_statement", "defer_statement"];
const SHORT_VAR: &str = "short_var_declaration";
const ASSIGNMENT: &str = "assignment_statement";
const DECLARATIONS: &[&str] = &["var_declaration", "const_declaration"];
const SPECS: &[&str] = &["var_spec", "const_spec"];
const IF: &str = "if_statement";
const FOR: &str = "for_statement";
const FOR_CLAUSE: &str = "for_clause";
const RANGE_CLAUSE: &str = "range_clause";
const SWITCHES: &[&str] = &[
    "expression_switch_statement",
    "type_switch_statement",
    "select_statement",
];
const CASES: &[&str] = &["expression_case", "type_case", "communication_case"];
const DEFAULT_CASE: &str = "default_case";
const LABELED: &str = "labeled_statement";
const RETURN: &str = "return_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const COMMENT: &str = "comment";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const INITIALIZER_FIELD: &str = "initializer";
const UPDATE_FIELD: &str = "update";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const COMPOUND_PLUS: &str = "+=";
const RETURN_SUBJECT: &str = "return";
// Dos vueltas alcanzan para que lo que entra en una vuelta llegue a la siguiente.
const LOOP_PASSES: usize = 2;

// Recorre una función con lo que se sabe de cada variable. Los caminos que salen (`return`,
// `panic` no) no siguen; un `break` o un `continue` terminan el bucle que los contiene.
pub struct Interpreter<'source, 'context> {
    pub(super) source: &'source [u8],
    pub(super) imports: &'context Imports,
    pub(super) types: Types,
    pub(super) returns: Flow,
    pub(super) calls: Vec<TaintCall>,
    pub(super) sinks: Vec<TaintSink>,
    pub(super) plain_response: bool,
    exits: Vec<Option<State>>,
}

impl<'source, 'context> Interpreter<'source, 'context> {
    pub fn new(source: &'source [u8], imports: &'context Imports, types: Types) -> Self {
        Self {
            source,
            imports,
            types,
            returns: Flow::default(),
            calls: Vec::new(),
            sinks: Vec::new(),
            plain_response: false,
            exits: Vec::new(),
        }
    }

    pub(super) fn text(&self, node: Node<'_>) -> &'source str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    // Si la función declara que la respuesta no es HTML, sus escrituras no son un XSS.
    pub fn run(&mut self, body: Node<'_>, state: State) {
        self.statement(body, state);
        if self.plain_response {
            self.sinks
                .retain(|sink| !matches!(sink.receiver, SinkReceiver::Known(SinkKind::Html)));
        }
    }

    fn statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        match node.kind() {
            BLOCK | STATEMENT_LIST => self.sequence(node, state),
            kind if EXPRESSION_STATEMENTS.contains(&kind) => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                Some(state)
            }
            SHORT_VAR | ASSIGNMENT => {
                let side = |name: &str| {
                    node.child_by_field_name(name)
                        .map(named_children)
                        .unwrap_or_default()
                };
                let operator = node
                    .child_by_field_name(OPERATOR_FIELD)
                    .map(|written| self.text(written));
                if operator == Some(COMPOUND_PLUS) {
                    self.append(&side(LEFT_FIELD), &side(RIGHT_FIELD), &mut state);
                } else {
                    self.assign(&side(LEFT_FIELD), &side(RIGHT_FIELD), &mut state);
                }
                Some(state)
            }
            kind if DECLARATIONS.contains(&kind) => {
                self.declarations(node, &mut state);
                Some(state)
            }
            IF => self.if_statement(node, state),
            FOR => self.repeat(node, state),
            kind if SWITCHES.contains(&kind) => self.switch(node, state),
            LABELED => match named_children(node).into_iter().last() {
                Some(inner) => self.statement(inner, state),
                None => Some(state),
            },
            RETURN => {
                self.return_statement(node, &mut state);
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

    fn sequence(&mut self, node: Node<'_>, state: State) -> Option<State> {
        named_children(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .try_fold(state, |current, child| self.statement(child, current))
    }

    fn declarations(&mut self, node: Node<'_>, state: &mut State) {
        let specs = crate::syntax::descendants(node)
            .into_iter()
            .filter(|spec| SPECS.contains(&spec.kind()));
        for spec in specs {
            let names: Vec<Node<'_>> = crate::syntax::children_with_fields(spec)
                .into_iter()
                .filter(|(field, _)| *field == Some("name"))
                .map(|(_, name)| name)
                .collect();
            let values = spec
                .child_by_field_name(VALUE_FIELD)
                .map(named_children)
                .unwrap_or_default();
            self.assign(&names, &values, state);
        }
    }

    // `a, b := x, y` va de a pares; `fila, err := consulta()` le da a cada uno todo lo que devuelve.
    // `texto += parte`: lo que había más lo que se suma.
    fn append(&mut self, left: &[Node<'_>], right: &[Node<'_>], state: &mut State) {
        let added = right.iter().fold(Flow::default(), |flow, node| {
            flow.union(&self.expression(*node, state).flow())
        });
        for target in left {
            let previous = self.expression(*target, state);
            self.store(*target, Value::Data(previous.flow().union(&added)), state);
        }
    }

    fn assign(&mut self, left: &[Node<'_>], right: &[Node<'_>], state: &mut State) {
        let values: Vec<Value> = right.iter().map(|node| self.expression(*node, state)).collect();
        let together = Value::Data(
            values
                .iter()
                .fold(Flow::default(), |flow, value| flow.union(&value.flow())),
        );
        if let ([target], [assigned]) = (left, right)
            && let Some(written) = self.type_of(*assigned)
        {
            self.types.declare(self.text(*target), written);
        }
        for (position, target) in left.iter().enumerate() {
            let value = if values.len() == left.len() {
                values.get(position).cloned().unwrap_or_else(Value::clean)
            } else {
                together.clone()
            };
            self.store(*target, value, state);
        }
    }

    fn if_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        if let Some(initializer) = node.child_by_field_name(INITIALIZER_FIELD) {
            state = self.statement(initializer, state)?;
        }
        let condition = node.child_by_field_name(CONDITION_FIELD)?;
        let decided = truth(self.expression(condition, &mut state).literal());
        let mut branch = |name: &str, current: State| match node.child_by_field_name(name) {
            Some(inner) => self.statement(inner, current),
            None => Some(current),
        };
        let taken = (decided != Some(false))
            .then(|| branch(CONSEQUENCE_FIELD, state.clone()))
            .flatten();
        let other = (decided != Some(true))
            .then(|| branch(ALTERNATIVE_FIELD, state))
            .flatten();
        join_states(taken, other)
    }

    fn repeat(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let clause = named_children(node)
            .into_iter()
            .find(|child| Some(*child) != node.child_by_field_name(BODY_FIELD));
        let field = |name: &str| clause.and_then(|inner| inner.child_by_field_name(name));
        let is = |kind: &str| clause.is_some_and(|inner| inner.kind() == kind);
        if let Some(initializer) = field(INITIALIZER_FIELD).filter(|_| is(FOR_CLAUSE)) {
            state = self.statement(initializer, state)?;
        }
        if is(RANGE_CLAUSE) {
            let targets = field(LEFT_FIELD).map(named_children).unwrap_or_default();
            let items = field(RIGHT_FIELD)
                .map_or_else(Flow::default, |items| self.expression(items, &mut state).flow());
            for target in targets {
                self.store(target, Value::Data(items.clone()), &mut state);
            }
        }
        let condition = if is(FOR_CLAUSE) {
            field(CONDITION_FIELD)
        } else {
            clause.filter(|_| !is(RANGE_CLAUSE))
        };
        self.exits.push(None);
        let mut entry = Some(state);
        for _ in 0..LOOP_PASSES {
            let Some(mut current) = entry.clone() else {
                break;
            };
            if let Some(test) = condition {
                self.expression(test, &mut current);
            }
            let mut after = node
                .child_by_field_name(BODY_FIELD)
                .and_then(|body| self.statement(body, current));
            if let (Some(update), Some(updated)) = (field(UPDATE_FIELD), after.take()) {
                after = self.statement(update, updated);
            }
            entry = join_states(entry, after);
        }
        let broken = self.exits.pop().flatten();
        join_states(entry, broken)
    }

    // Con el valor del `switch` constante, sólo corre el caso que toca; si no, todos. En Go un caso
    // no sigue en el próximo.
    fn switch(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        if let Some(initializer) = node.child_by_field_name(INITIALIZER_FIELD) {
            state = self.statement(initializer, state)?;
        }
        let subject = node
            .child_by_field_name(VALUE_FIELD)
            .map(|written| self.expression(written, &mut state))
            .and_then(|value| value.literal().cloned());
        let cases: Vec<Node<'_>> = named_children(node)
            .into_iter()
            .filter(|case| CASES.contains(&case.kind()) || case.kind() == DEFAULT_CASE)
            .collect();
        let matched = subject.as_ref().and_then(|value| {
            cases
                .iter()
                .position(|case| {
                    case.child_by_field_name(VALUE_FIELD)
                        .map(named_children)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|label| literal_of(label, self.source))
                        .any(|label| same_label(value, &label))
                })
                .or_else(|| cases.iter().position(|case| case.kind() == DEFAULT_CASE))
        });
        self.exits.push(None);
        let mut joined = None;
        for (position, case) in cases.iter().enumerate() {
            if subject.is_some() && matched != Some(position) {
                continue;
            }
            let ran = named_children(*case)
                .into_iter()
                .filter(|child| child.kind() == STATEMENT_LIST)
                .try_fold(state.clone(), |current, list| self.statement(list, current));
            joined = join_states(joined, ran);
        }
        let broken = self.exits.pop().flatten();
        let has_default = cases.iter().any(|case| case.kind() == DEFAULT_CASE);
        let skipped = (!has_default && (subject.is_none() || matched.is_none())).then_some(state);
        join_states(join_states(joined, broken), skipped)
    }

    // Lo que se devuelve; una respuesta de Lambda que se declara HTML es un sumidero.
    fn return_statement(&mut self, node: Node<'_>, state: &mut State) {
        let returned = named_children(node).into_iter().flat_map(named_children);
        for child in returned {
            let value = self.expression(child, state);
            if let Some(body) = lambda_html_body(&value) {
                self.sinks
                    .push(sink_at(node, RETURN_SUBJECT, SinkKind::Html, vec![body]));
            }
            self.returns.absorb(&value.flow());
        }
    }
}
