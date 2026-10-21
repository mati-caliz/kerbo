use crate::shell::flow::commands::Commands;
use crate::shell::flow::values::Values;
use tree_sitter::Node;

use super::Interpreter;
use super::state::{State, Variable};
use super::values::{is_splitting_word, word_value};
use crate::shell::syntax::{
    ARITHMETIC_FOR, ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, CASE, CASE_ITEM, COMMAND, COMMENT, COMPOUND,
    CONDITION_FIELD, DECLARATION, DO_GROUP, ELIF_CLAUSE, ELSE_CLAUSE, FOR, FUNCTION, IF, LIST, NEGATED,
    PIPELINE, PROGRAM, REDIRECT_FIELD, REDIRECTED, SUBSHELL, UNSET, VALUE_FIELD, VARIABLE_FIELD, WHILE,
    children, field_children, is_arithmetic_command, is_backgrounded, named_children, text,
};

const THEN: &str = "then";
const FALLTHROUGH: &str = ";&";
const CONTINUE_MATCHING: &str = ";;&";
const CATCH_ALL: char = '*';
const MAX_LOOP_ROUNDS: usize = 64;

pub trait Control<'tree> {
    fn statements(&mut self, nodes: &[Node<'tree>], state: State) -> State;
    fn statement(&mut self, node: Node<'tree>, state: State) -> State;
    fn redirected(&mut self, node: Node<'tree>, state: State) -> State;
    fn pipeline(&mut self, node: Node<'tree>, state: State) -> State;
    fn list(&mut self, node: Node<'tree>, state: State) -> State;
    fn if_statement(&mut self, node: Node<'tree>, state: State) -> State;
    fn while_statement(&mut self, node: Node<'tree>, state: State) -> State;
    fn for_statement(&mut self, node: Node<'tree>, state: State) -> State;
    fn arithmetic_for(&mut self, node: Node<'tree>, state: State) -> State;
    fn case_statement(&mut self, node: Node<'tree>, state: State) -> State;
}

impl<'tree> Control<'tree> for Interpreter<'tree, '_, '_> {
    fn statements(&mut self, nodes: &[Node<'tree>], state: State) -> State {
        let mut state = state;
        for node in nodes {
            if node.kind() == COMMENT || !node.is_named() {
                continue;
            }
            if is_backgrounded(*node, self.source()) {
                self.isolated(*node, &state);
                continue;
            }
            state = self.statement(*node, state);
        }
        state
    }

    fn statement(&mut self, node: Node<'tree>, state: State) -> State {
        match node.kind() {
            COMPOUND if is_arithmetic_command(node, self.source()) => {
                self.arithmetic(&named_children(node), state)
            }
            PROGRAM | DO_GROUP | NEGATED | COMPOUND => self.statements(&named_children(node), state),
            COMMAND | DECLARATION | UNSET => self.command(node, state),
            ASSIGNMENT => self.assignment(node, state, super::commands::AssignmentScope::Variable),
            ASSIGNMENTS => named_children(node).into_iter().fold(state, |state, assignment| {
                self.assignment(assignment, state, super::commands::AssignmentScope::Variable)
            }),
            REDIRECTED => self.redirected(node, state),
            PIPELINE => self.pipeline(node, state),
            LIST => self.list(node, state),
            SUBSHELL => {
                self.subshell(&named_children(node), &state);
                state
            }
            IF => self.if_statement(node, state),
            WHILE => self.while_statement(node, state),
            FOR => self.for_statement(node, state),
            ARITHMETIC_FOR => self.arithmetic_for(node, state),
            CASE => self.case_statement(node, state),
            FUNCTION => {
                let mut state = state;
                self.declare_function(node, &mut state);
                state
            }
            _ => self.expand_all(&named_children(node), state),
        }
    }

    fn redirected(&mut self, node: Node<'tree>, state: State) -> State {
        let redirects = field_children(node, REDIRECT_FIELD);
        let state = self.expand_all(&redirects, state);
        match node.child_by_field_name(BODY_FIELD) {
            Some(body) => self.statement(body, state),
            None => state,
        }
    }

    fn pipeline(&mut self, node: Node<'tree>, state: State) -> State {
        let elements = named_children(node);
        let Some((last, leading)) = elements.split_last() else {
            return state;
        };
        for element in leading {
            self.isolated(*element, &state);
        }
        if self.lastpipe {
            self.statement(*last, state)
        } else {
            self.isolated(*last, &state);
            state
        }
    }

    fn list(&mut self, node: Node<'tree>, state: State) -> State {
        let operands = named_children(node);
        let Some((first, rest)) = operands.split_first() else {
            return state;
        };
        let left = self.statement(*first, state);
        let right = self.statements(rest, left.clone());
        self.stack.merge(vec![left, right])
    }

    fn if_statement(&mut self, node: Node<'tree>, state: State) -> State {
        let conditions: Vec<Node<'tree>> = field_children(node, CONDITION_FIELD);
        let (_, then_body) = split_elif(node, self.source());
        let then_body: Vec<Node<'tree>> = then_body
            .into_iter()
            .filter(|child| ![ELIF_CLAUSE, ELSE_CLAUSE].contains(&child.kind()))
            .collect();
        let mut failed = self.statements(&conditions, state);
        let mut branches = vec![self.statements(&then_body, failed.clone())];
        for clause in children(node) {
            if clause.kind() == ELIF_CLAUSE {
                let (condition, body) = split_elif(clause, self.source());
                failed = self.statements(&condition, failed);
                branches.push(self.statements(&body, failed.clone()));
            } else if clause.kind() == ELSE_CLAUSE {
                failed = self.statements(&named_children(clause), failed);
            }
        }
        branches.push(failed);
        self.stack.merge(branches)
    }

    fn while_statement(&mut self, node: Node<'tree>, state: State) -> State {
        let conditions = field_children(node, CONDITION_FIELD);
        let body = node.child_by_field_name(BODY_FIELD);
        let mut head = state.clone();
        let mut exit = state;
        for _ in 0..MAX_LOOP_ROUNDS {
            let checked = self.statements(&conditions, head.clone());
            let looped = body.map_or_else(|| checked.clone(), |body| self.statement(body, checked.clone()));
            exit = checked;
            let next = self.stack.merge(vec![head.clone(), looped]);
            if next == head {
                break;
            }
            head = next;
        }
        exit
    }

    fn for_statement(&mut self, node: Node<'tree>, state: State) -> State {
        let words = field_children(node, VALUE_FIELD);
        let expanded = self.expand_all(&words, state);
        let Some(variable) = node.child_by_field_name(VARIABLE_FIELD) else {
            return expanded;
        };
        let name = text(variable, self.source()).to_owned();
        let values: Vec<Variable> =
            if words.is_empty() || words.iter().any(|word| is_splitting_word(*word, self.source())) {
                vec![Variable::UNKNOWN]
            } else {
                words
                    .iter()
                    .map(|word| word_value(self, *word, &expanded))
                    .collect()
            };
        let body = node.child_by_field_name(BODY_FIELD);
        let mut choice = expanded.clone();
        let mut exit = expanded.clone();
        for _ in 0..MAX_LOOP_ROUNDS {
            let assigned: Vec<State> = values
                .iter()
                .map(|value| {
                    let mut assigned = choice.clone();
                    self.stack.write(&mut assigned, &name, *value);
                    assigned
                })
                .collect();
            let entered = self.stack.merge(assigned);
            let looped = match body {
                Some(body) => self.statement(body, entered),
                None => entered,
            };
            exit = self.stack.merge(vec![expanded.clone(), looped.clone()]);
            let next = self.stack.merge(vec![expanded.clone(), looped]);
            if next == choice {
                break;
            }
            choice = next;
        }
        exit
    }

    fn arithmetic_for(&mut self, node: Node<'tree>, state: State) -> State {
        let section = |field: &str| field_children(node, field);
        let initialized = self.arithmetic(&section("initializer"), state);
        let body = node.child_by_field_name(BODY_FIELD);
        let mut head = initialized;
        let mut exit = head.clone();
        for _ in 0..MAX_LOOP_ROUNDS {
            let checked = self.arithmetic(&section(CONDITION_FIELD), head.clone());
            let looped = body.map_or_else(|| checked.clone(), |body| self.statement(body, checked.clone()));
            let incremented = self.arithmetic(&section("update"), looped);
            exit = checked;
            let next = self.stack.merge(vec![head.clone(), incremented]);
            if next == head {
                break;
            }
            head = next;
        }
        exit
    }

    fn case_statement(&mut self, node: Node<'tree>, state: State) -> State {
        let subject = field_children(node, VALUE_FIELD);
        let matched = self.expand_all(&subject, state);
        let items: Vec<Node<'tree>> = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == CASE_ITEM)
            .collect();
        let mut finished = Vec::new();
        let mut next_condition = matched.clone();
        let mut carried: Option<State> = None;
        let mut has_catch_all = false;
        let last = items.last().copied();
        for item in items {
            let patterns = field_children(item, VALUE_FIELD);
            has_catch_all |= patterns
                .iter()
                .any(|pattern| is_catch_all(*pattern, self.source()));
            let tested = self.expand_all(&patterns, next_condition.clone());
            next_condition = tested.clone();
            let entry = match carried.take() {
                Some(previous) => self.stack.merge(vec![tested, previous]),
                None => tested,
            };
            let body: Vec<Node<'tree>> = named_children(item)
                .into_iter()
                .filter(|child| !patterns.contains(child))
                .collect();
            let done = self.statements(&body, entry);
            if Some(item) == last {
                finished.push(done);
                continue;
            }
            match terminator(item, self.source()).as_deref() {
                Some(FALLTHROUGH) => carried = Some(done),
                Some(CONTINUE_MATCHING) => next_condition = self.stack.merge(vec![next_condition, done]),
                _ => finished.push(done),
            }
        }
        if !has_catch_all {
            finished.push(matched);
        }
        self.stack.merge(finished)
    }
}

fn split_elif<'tree>(clause: Node<'tree>, source: &[u8]) -> (Vec<Node<'tree>>, Vec<Node<'tree>>) {
    let mut condition = Vec::new();
    let mut body = Vec::new();
    let mut after_then = false;
    for child in children(clause) {
        if text(child, source) == THEN {
            after_then = true;
        } else if child.is_named() && child.kind() != COMMENT {
            if after_then {
                body.push(child);
            } else {
                condition.push(child);
            }
        }
    }
    (condition, body)
}

fn terminator(item: Node<'_>, source: &[u8]) -> Option<String> {
    children(item)
        .into_iter()
        .rev()
        .find(|child| !child.is_named())
        .map(|child| text(child, source).to_owned())
}

fn is_catch_all(pattern: Node<'_>, source: &[u8]) -> bool {
    let raw = text(pattern, source);
    !raw.is_empty() && raw.chars().all(|character| character == CATCH_ALL)
}
