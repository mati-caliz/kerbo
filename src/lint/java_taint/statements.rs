use std::collections::HashMap;

use tree_sitter::Node;

use super::constants::{literal_of, same_label, truth};
use super::receivers::sink_receiver;
use super::values::{Flow, Literal, State, Value, join_states};
use super::{SinkReceiver, TaintCall, TaintSink};
use crate::lint::java::{NAME_FIELD, text};
use crate::lint::java_receivers::{StaticImports, Variables};
use crate::syntax::named_children;

const BLOCK: &str = "block";
const INVOCATION: &str = "method_invocation";
const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const IF: &str = "if_statement";
const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const FOR: &str = "for_statement";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const RESOURCES: &str = "resource_specification";
const RESOURCE: &str = "resource";
const CATCH: &str = "catch_clause";
const FINALLY: &str = "finally_clause";
pub(super) const SWITCH: &str = "switch_expression";
const SWITCH_GROUP: &str = "switch_block_statement_group";
const SWITCH_RULE: &str = "switch_rule";
pub(super) const SWITCH_LABEL: &str = "switch_label";
const SYNCHRONIZED: &str = "synchronized_statement";
const LABELED: &str = "labeled_statement";
const RETURN: &str = "return_statement";
const BREAK: &str = "break_statement";
const CONTINUE: &str = "continue_statement";
const EXITS: &[&str] = &["throw_statement", "yield_statement"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
pub(super) const BODY_FIELD: &str = "body";
const INIT_FIELD: &str = "init";
const UPDATE_FIELD: &str = "update";
const VALUE_FIELD: &str = "value";
const DEFAULT_LABEL: &str = "default";
// Dos vueltas alcanzan para que lo que entra en una vuelta llegue a la siguiente.
const LOOP_PASSES: usize = 2;

// Recorre un método con lo que se sabe de cada variable. Los caminos que salen (`return`,
// `throw`) no siguen; un `break` o un `continue` termina el bucle o el `switch` que lo contiene.
pub struct Interpreter<'source, 'context> {
    pub(super) source: &'source [u8],
    pub(super) variables: &'context Variables,
    pub(super) imported: &'context StaticImports,
    pub(super) returns: Flow,
    pub(super) calls: Vec<TaintCall>,
    pub(super) sinks: Vec<TaintSink>,
    pub(super) factory_variables: HashMap<String, String>,
    exits: Vec<Option<State>>,
}

impl<'source, 'context> Interpreter<'source, 'context> {
    pub fn new(
        source: &'source [u8],
        variables: &'context Variables,
        imported: &'context StaticImports,
    ) -> Self {
        Self {
            source,
            variables,
            imported,
            returns: Flow::default(),
            calls: Vec::new(),
            sinks: Vec::new(),
            factory_variables: HashMap::new(),
            exits: Vec::new(),
        }
    }

    pub(super) fn text(&self, node: Node<'_>) -> &'source str {
        text(node, self.source)
    }

    pub fn run(&mut self, body: Node<'_>, state: State) {
        self.statement(body, state);
    }

    fn statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let field = |name: &str| node.child_by_field_name(name);
        match node.kind() {
            BLOCK => self.sequence(&named_children(node), state),
            LOCAL => {
                self.declaration(node, &mut state);
                Some(state)
            }
            EXPRESSION_STATEMENT => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                Some(state)
            }
            IF => {
                let condition = self.expression(field(CONDITION_FIELD)?, &mut state);
                let decided = truth(condition.literal());
                let taken = (decided != Some(false))
                    .then(|| self.branch(field(CONSEQUENCE_FIELD), state.clone()))
                    .flatten();
                let other = (decided != Some(true))
                    .then(|| self.branch(field(ALTERNATIVE_FIELD), state))
                    .flatten();
                join_states(taken, other)
            }
            WHILE | FOR | DO | ENHANCED_FOR => self.repeat(node, state),
            SWITCH => self.switch_statement(node, state),
            SYNCHRONIZED | LABELED => {
                let parts: Vec<Node<'_>> = named_children(node)
                    .into_iter()
                    .filter(|child| child.kind() == BLOCK || child.kind().ends_with("statement"))
                    .collect();
                self.sequence(&parts, state)
            }
            RETURN => {
                for child in named_children(node) {
                    let returned = self.expression(child, &mut state).flow();
                    self.returns.absorb(&returned);
                }
                None
            }
            BREAK | CONTINUE => {
                if let Some(exit) = self.exits.last_mut() {
                    *exit = join_states(exit.take(), Some(state));
                }
                None
            }
            kind if TRY_STATEMENTS.contains(&kind) => self.try_statement(node, state),
            kind if EXITS.contains(&kind) => {
                for child in named_children(node) {
                    self.expression(child, &mut state);
                }
                None
            }
            _ => Some(state),
        }
    }

    fn branch(&mut self, branch: Option<Node<'_>>, state: State) -> Option<State> {
        let Some(node) = branch else {
            return Some(state);
        };
        self.statement(node, state)
    }

    fn sequence(&mut self, nodes: &[Node<'_>], state: State) -> Option<State> {
        nodes
            .iter()
            .filter(|child| !COMMENTS.contains(&child.kind()))
            .try_fold(state, |current, child| self.statement(*child, current))
    }

    fn declaration(&mut self, node: Node<'_>, state: &mut State) {
        for declarator in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        {
            let (Some(name), Some(value)) = (
                declarator.child_by_field_name(NAME_FIELD),
                declarator.child_by_field_name(VALUE_FIELD),
            ) else {
                continue;
            };
            let assigned = self.expression(value, state);
            let variable = self.text(name).to_owned();
            if value.kind() == INVOCATION
                && let Some(SinkReceiver::Type(known)) = sink_receiver(self, value)
            {
                self.factory_variables.insert(variable.clone(), known);
            }
            state.insert(variable, assigned);
        }
    }

    // Un bucle: el cuerpo corre desde la entrada y desde el final de la vuelta anterior; sale por la
    // condición o por un `break`.
    fn repeat(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let field = |name: &str| node.child_by_field_name(name);
        if let Some(init) = field(INIT_FIELD) {
            if init.kind() == LOCAL {
                self.declaration(init, &mut state);
            } else {
                self.expression(init, &mut state);
            }
        }
        if node.kind() == ENHANCED_FOR
            && let (Some(name), Some(iterable)) = (field(NAME_FIELD), field(VALUE_FIELD))
        {
            let items = self.expression(iterable, &mut state);
            state.insert(self.text(name).to_owned(), Value::Data(items.flow()));
        }
        self.exits.push(None);
        let mut entry = Some(state);
        for _ in 0..LOOP_PASSES {
            let Some(mut current) = entry.clone() else {
                break;
            };
            if let Some(condition) = field(CONDITION_FIELD).filter(|_| node.kind() != DO) {
                self.expression(condition, &mut current);
            }
            let mut after = field(BODY_FIELD).and_then(|body| self.statement(body, current));
            if let (Some(update), Some(updated)) = (field(UPDATE_FIELD), after.as_mut()) {
                self.expression(update, updated);
            }
            entry = join_states(entry, after);
        }
        let broken = self.exits.pop().flatten();
        join_states(entry, broken)
    }

    // Con el valor del `switch` constante, sólo corre el caso que toca (y los que siguen hasta un
    // `break`); si no, todos.
    pub(super) fn switch_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        let subject = node
            .child_by_field_name(CONDITION_FIELD)
            .map(|condition| self.expression(condition, &mut state));
        let subject = subject.as_ref().and_then(Value::literal).cloned();
        let cases: Vec<Node<'_>> = node
            .child_by_field_name(BODY_FIELD)
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|case| case.kind() == SWITCH_GROUP || case.kind() == SWITCH_RULE)
            .collect();
        let matched = subject.as_ref().and_then(|value| {
            cases
                .iter()
                .position(|case| self.labels(*case).iter().any(|label| same_label(value, label)))
                .or_else(|| cases.iter().position(|case| self.is_default(*case)))
        });
        let has_default = cases.iter().any(|case| self.is_default(*case));
        self.exits.push(None);
        let mut falling: Option<State> = None;
        let mut finished: Option<State> = None;
        for (position, case) in cases.iter().enumerate() {
            let enters = match (&subject, matched) {
                (Some(_), Some(found)) => position == found,
                (Some(_), None) => false,
                (None, _) => true,
            };
            let entry = join_states(falling.take(), enters.then(|| state.clone()));
            let Some(entry) = entry else {
                continue;
            };
            let statements: Vec<Node<'_>> = named_children(*case)
                .into_iter()
                .filter(|child| child.kind() != SWITCH_LABEL)
                .collect();
            let after = self.sequence(&statements, entry);
            if case.kind() == SWITCH_RULE {
                finished = join_states(finished, after);
            } else {
                falling = after;
            }
        }
        let broken = self.exits.pop().flatten();
        let skipped = (!has_default && (subject.is_none() || matched.is_none())).then_some(state);
        join_states(join_states(join_states(finished, falling), broken), skipped)
    }

    fn labels(&self, case: Node<'_>) -> Vec<Literal> {
        named_children(case)
            .into_iter()
            .filter(|child| child.kind() == SWITCH_LABEL)
            .flat_map(named_children)
            .filter_map(|label| literal_of(label, self.source))
            .collect()
    }

    fn is_default(&self, case: Node<'_>) -> bool {
        named_children(case)
            .into_iter()
            .filter(|child| child.kind() == SWITCH_LABEL)
            .any(|label| self.text(label).trim() == DEFAULT_LABEL)
    }

    fn resource(&mut self, resource: Node<'_>, state: &mut State) {
        let (Some(name), Some(value)) = (
            resource.child_by_field_name(NAME_FIELD),
            resource.child_by_field_name(VALUE_FIELD),
        ) else {
            self.expression(resource, state);
            return;
        };
        let opened = self.expression(value, state);
        state.insert(self.text(name).to_owned(), opened);
    }

    // El `finally` puede correr después de una excepción en cualquier punto del `try`: arranca también
    // con lo que había a la entrada.
    fn try_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        for resources in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == RESOURCES)
        {
            for resource in named_children(resources)
                .into_iter()
                .filter(|child| child.kind() == RESOURCE)
            {
                self.resource(resource, &mut state);
            }
        }
        let mut result = node
            .child_by_field_name(BODY_FIELD)
            .and_then(|body| self.statement(body, state.clone()));
        let mut finally = None;
        for clause in named_children(node) {
            match clause.kind() {
                CATCH => {
                    let entry = join_states(Some(state.clone()), result.clone());
                    let handled = clause
                        .child_by_field_name(BODY_FIELD)
                        .zip(entry)
                        .and_then(|(block, entry)| self.statement(block, entry));
                    result = join_states(result, handled);
                }
                FINALLY => {
                    finally = named_children(clause)
                        .into_iter()
                        .find(|child| child.kind() == BLOCK);
                }
                _ => {}
            }
        }
        match finally {
            Some(block) => {
                let entry = join_states(Some(state), result.clone()).unwrap_or_default();
                self.statement(block, entry).and(result)
            }
            None => result,
        }
    }
}
