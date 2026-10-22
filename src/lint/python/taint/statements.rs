use std::collections::HashMap;

use tree_sitter::Node;

use super::imports::Imports;
use crate::lint::taint::folding::{same_label, truth};
use crate::lint::taint::responses::lambda_html_body;
use crate::lint::taint::{Flow, Literal, SinkKind, State, TaintCall, TaintSink, Value, join_states};
use crate::syntax::named_children;

const BLOCK: &str = "block";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const ELSE: &str = "else_clause";
const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const TRY: &str = "try_statement";
const EXCEPT: &str = "except_clause";
const FINALLY: &str = "finally_clause";
const WITH: &str = "with_statement";
const WITH_CLAUSE: &str = "with_clause";
const WITH_ITEM: &str = "with_item";
const AS_PATTERN: &str = "as_pattern";
const MATCH: &str = "match_statement";
const CASE: &str = "case_clause";
const CASE_PATTERN: &str = "case_pattern";
const RETURN: &str = "return_statement";
const RETURN_SUBJECT: &str = "return";
// Lo que una vista devuelve como JSON y no como HTML.
const JSON_RESULTS: &[&str] = &[
    "dictionary",
    "list",
    "list_comprehension",
    "dictionary_comprehension",
];
const RAISE: &str = "raise_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const DEFINITIONS: &[&str] = &["function_definition", "class_definition", "decorated_definition"];
const COMPARISON: &str = "comparison_operator";
const IDENTIFIER: &str = "identifier";
const STRING: &str = "string";
const IN: &str = "in";
const WILDCARD: &str = "_";
const UNION_SEPARATOR: char = '|';
const PARENT_DIRECTORY: &str = "..";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const BODY_FIELD: &str = "body";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const VALUE_FIELD: &str = "value";
const ALIAS_FIELD: &str = "alias";
const SUBJECT_FIELD: &str = "subject";
const OPERATORS_FIELD: &str = "operators";
// Dos vueltas alcanzan para que lo que entra en una vuelta llegue a la siguiente.
const LOOP_PASSES: usize = 2;

// Recorre una función con lo que se sabe de cada variable. Los caminos que salen (`return`,
// `raise`) no siguen; un `break` o un `continue` terminan el bucle que los contiene.
pub struct Interpreter<'source, 'context> {
    pub(super) source: &'source [u8],
    pub(super) imports: &'context Imports,
    pub(super) locals: &'context HashMap<String, String>,
    pub(super) class: Option<&'context str>,
    pub(super) route: bool,
    pub(super) route_path: Option<String>,
    pub(super) returns: Flow,
    pub(super) calls: Vec<TaintCall>,
    pub(super) sinks: Vec<TaintSink>,
    pub(super) instances: HashMap<String, String>,
    exits: Vec<Option<State>>,
}

impl<'source, 'context> Interpreter<'source, 'context> {
    pub fn new(
        source: &'source [u8],
        imports: &'context Imports,
        locals: &'context HashMap<String, String>,
        class: Option<&'context str>,
    ) -> Self {
        Self {
            source,
            imports,
            locals,
            class,
            route: false,
            route_path: None,
            returns: Flow::default(),
            calls: Vec::new(),
            sinks: Vec::new(),
            instances: HashMap::new(),
            exits: Vec::new(),
        }
    }

    pub(super) fn text(&self, node: Node<'_>) -> &'source str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    pub fn run(&mut self, body: Node<'_>, state: State) {
        self.statement(body, state);
    }

    fn statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let field = |name: &str| node.child_by_field_name(name);
        match node.kind() {
            BLOCK | ELSE | FINALLY => self.sequence(node, state),
            EXPRESSION_STATEMENT => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                Some(state)
            }
            IF => self.if_statement(node, state),
            FOR | WHILE => self.repeat(node, state),
            TRY => self.try_statement(node, state),
            WITH => {
                for item in named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == WITH_CLAUSE)
                    .flat_map(named_children)
                    .filter(|child| child.kind() == WITH_ITEM)
                {
                    self.with_item(item, &mut state);
                }
                field(BODY_FIELD).and_then(|body| self.statement(body, state))
            }
            MATCH => self.match_statement(node, state),
            RETURN => {
                for child in named_children(node) {
                    self.returned(node, child, &mut state);
                }
                None
            }
            RAISE => {
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
            kind if DEFINITIONS.contains(&kind) => Some(state),
            _ => Some(state),
        }
    }

    // Lo que devuelve la función; en una vista de Flask, también el cuerpo de la respuesta, salvo que
    // sea JSON.
    fn returned(&mut self, statement: Node<'_>, child: Node<'_>, state: &mut State) {
        let returned = self.expression(child, state);
        if let Some(body) = lambda_html_body(&returned) {
            self.sinks.push(super::expressions::sink_at(
                statement,
                RETURN_SUBJECT,
                SinkKind::Html,
                vec![body],
            ));
        }
        if self.route && !JSON_RESULTS.contains(&child.kind()) && !matches!(returned, Value::Map(_)) {
            let body = super::expressions::response_body(child, &returned);
            self.sinks.push(super::expressions::sink_at(
                statement,
                RETURN_SUBJECT,
                SinkKind::Html,
                vec![body],
            ));
        }
        self.returns.absorb(&returned.flow());
    }

    fn sequence(&mut self, node: Node<'_>, state: State) -> Option<State> {
        named_children(node)
            .into_iter()
            .try_fold(state, |current, child| self.statement(child, current))
    }

    fn branch(&mut self, branch: Option<Node<'_>>, state: State) -> Option<State> {
        let Some(node) = branch else {
            return Some(state);
        };
        self.statement(node, state)
    }

    // Un `if` con sus `elif`: cada condición corre si las anteriores pudieron ser falsas, y con una
    // constante sólo la rama que toca. Un `if '..' in ruta: return` deja la ruta validada.
    fn if_statement(&mut self, node: Node<'_>, state: State) -> Option<State> {
        let mut clauses = vec![node];
        clauses.extend(
            named_children(node)
                .into_iter()
                .filter(|child| child.kind() == ELIF),
        );
        let otherwise = named_children(node)
            .into_iter()
            .find(|child| child.kind() == ELSE);
        let mut result = None;
        let mut remaining = Some(state);
        for clause in clauses {
            let Some(mut current) = remaining.take() else {
                break;
            };
            let Some(condition) = clause.child_by_field_name(CONDITION_FIELD) else {
                remaining = Some(current);
                continue;
            };
            let decided = truth(self.expression(condition, &mut current).literal());
            let taken = (decided != Some(false))
                .then(|| self.branch(clause.child_by_field_name(CONSEQUENCE_FIELD), current.clone()))
                .flatten();
            let exits = decided != Some(false) && taken.is_none();
            result = join_states(result, taken);
            remaining = (decided != Some(true)).then(|| self.validated(condition, exits, current));
        }
        let finished = match (otherwise, remaining) {
            (Some(clause), Some(current)) => self.statement(clause, current),
            (None, current) => current,
            (Some(_), None) => None,
        };
        join_states(result, finished)
    }

    fn validated(&self, condition: Node<'_>, consequence_exits: bool, mut state: State) -> State {
        if consequence_exits
            && let Some(checked) = self.checked_path(condition)
            && let Some(value) = state.remove(checked)
        {
            state.insert(
                checked.to_owned(),
                Value::Data(value.flow().without(SinkKind::Path.bit())),
            );
        }
        state
    }

    // `'..' in ruta`: la variable que se chequea contra la subida de directorio.
    fn checked_path(&self, condition: Node<'_>) -> Option<&'source str> {
        if condition.kind() != COMPARISON {
            return None;
        }
        let parts = named_children(condition);
        let [needle, haystack] = parts.as_slice() else {
            return None;
        };
        let operator = condition
            .child_by_field_name(OPERATORS_FIELD)
            .map(|written| self.text(written));
        (operator == Some(IN)
            && needle.kind() == STRING
            && self.text(*needle).contains(PARENT_DIRECTORY)
            && haystack.kind() == IDENTIFIER)
            .then(|| self.text(*haystack))
    }

    fn repeat(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        if let (Some(target), Some(iterable)) = (
            node.child_by_field_name(LEFT_FIELD),
            node.child_by_field_name(RIGHT_FIELD),
        ) {
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
            let after = node
                .child_by_field_name(BODY_FIELD)
                .and_then(|body| self.statement(body, current));
            entry = join_states(entry, after);
        }
        let broken = self.exits.pop().flatten();
        let finished = named_children(node)
            .into_iter()
            .find(|child| child.kind() == ELSE)
            .map_or_else(
                || entry.clone(),
                |otherwise| {
                    entry
                        .clone()
                        .and_then(|current| self.statement(otherwise, current))
                },
            );
        join_states(finished, broken)
    }

    fn with_item(&mut self, item: Node<'_>, state: &mut State) {
        let Some(value) = item.child_by_field_name(VALUE_FIELD) else {
            return;
        };
        if value.kind() == AS_PATTERN {
            let opened = named_children(value)
                .first()
                .map_or_else(Value::clean, |inner| self.expression(*inner, state));
            if let Some(target) = value.child_by_field_name(ALIAS_FIELD) {
                self.bind(target, &opened, state);
            }
        } else {
            self.expression(value, state);
        }
    }

    // El `finally` puede correr después de una excepción en cualquier punto del `try`: arranca también
    // con lo que había a la entrada.
    fn try_statement(&mut self, node: Node<'_>, state: State) -> Option<State> {
        let mut result = node
            .child_by_field_name(BODY_FIELD)
            .and_then(|body| self.statement(body, state.clone()));
        let mut finally = None;
        for clause in named_children(node) {
            match clause.kind() {
                EXCEPT => {
                    let entry = join_states(Some(state.clone()), result.clone());
                    let handled = named_children(clause)
                        .into_iter()
                        .find(|child| child.kind() == BLOCK)
                        .zip(entry)
                        .and_then(|(block, entry)| self.statement(block, entry));
                    result = join_states(result, handled);
                }
                ELSE => {
                    result = result.and_then(|current| self.statement(clause, current));
                }
                FINALLY => finally = Some(clause),
                _ => {}
            }
        }
        let Some(clause) = finally else {
            return result;
        };
        let entry = join_states(Some(state), result.clone()).unwrap_or_default();
        self.statement(clause, entry).and(result)
    }

    // Con el valor del `match` constante, sólo corre el primer caso que lo acepta; si no, todos.
    fn match_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let subject = node
            .child_by_field_name(SUBJECT_FIELD)
            .map(|written| self.expression(written, &mut state))
            .and_then(|value| value.literal().cloned());
        let cases: Vec<Node<'_>> = node
            .child_by_field_name(BODY_FIELD)
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|child| child.kind() == CASE)
            .collect();
        let chosen = subject
            .as_ref()
            .and_then(|value| cases.iter().position(|case| self.case_accepts(*case, value)));
        let mut result = None;
        for (position, case) in cases.iter().enumerate() {
            if subject.is_some() && chosen != Some(position) {
                continue;
            }
            let body = case.child_by_field_name(CONSEQUENCE_FIELD);
            result = join_states(result, self.branch(body, state.clone()));
        }
        if subject.is_none() || chosen.is_none() {
            result = join_states(result, Some(state));
        }
        result
    }

    fn case_accepts(&self, case: Node<'_>, subject: &Literal) -> bool {
        named_children(case)
            .into_iter()
            .filter(|child| child.kind() == CASE_PATTERN)
            .any(|pattern| {
                let written = self.text(pattern).trim();
                written == WILDCARD
                    || written.split(UNION_SEPARATOR).map(str::trim).any(|option| {
                        super::expressions::literal_of_text(option)
                            .is_some_and(|label| same_label(subject, &label))
                    })
            })
    }
}
