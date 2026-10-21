use tree_sitter::Node;

use super::Interpreter;
use super::state::{State, Variable};
use crate::shell::parameters::{is_array_expansion, modifier, reference};
use crate::shell::syntax::{
    ANSI_STRING, ARITHMETIC_EXPANSION, ASSIGNMENT, BINARY, COMMAND_SUBSTITUTION, CONCATENATION, EXPANSION,
    EXPANSIONS, INDEX_FIELD, LEFT_FIELD, NAME_FIELD, NUMBER, OPERATOR_FIELD, POSTFIX, PROCESS_SUBSTITUTION,
    RAW_STRING, RIGHT_FIELD, SIMPLE_EXPANSION, STRING, STRING_CONTENT, SUBSCRIPT, TERNARY, UNARY,
    VALUE_FIELD, VARIABLE_NAME, WORD, children, named_children, text, token_text,
};
use crate::shell::words::{braced_content, is_brace_expansion, literal};

const DEFAULT_ASSIGNMENTS: &[&str] = &["=", ":="];
const ASSIGNMENT_OPERATORS: &[&str] = &["=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|="];
const STEPS: &[&str] = &["++", "--"];
const GLOB_CHARACTERS: &[char] = &['*', '?', '['];
const ESCAPE: char = '\\';

pub trait Values<'tree> {
    fn expand_all(&mut self, nodes: &[Node<'tree>], state: State) -> State;
    fn expand(&mut self, node: Node<'tree>, state: State) -> State;
    fn parameter(&mut self, node: Node<'tree>, state: State) -> State;
    fn arithmetic(&mut self, nodes: &[Node<'tree>], state: State) -> State;
    fn arithmetic_node(&mut self, node: Node<'tree>, state: State) -> State;
    fn arithmetic_write(&mut self, target: Option<Node<'tree>>, state: State) -> State;
}

impl<'tree> Values<'tree> for Interpreter<'tree, '_, '_> {
    fn expand_all(&mut self, nodes: &[Node<'tree>], state: State) -> State {
        nodes.iter().fold(state, |state, node| self.expand(*node, state))
    }

    fn expand(&mut self, node: Node<'tree>, state: State) -> State {
        match node.kind() {
            SIMPLE_EXPANSION | EXPANSION => self.parameter(node, state),
            COMMAND_SUBSTITUTION | PROCESS_SUBSTITUTION => {
                let body = named_children(node);
                self.subshell(&body, &state);
                state
            }
            ARITHMETIC_EXPANSION => self.arithmetic(&named_children(node), state),
            _ => self.expand_all(&named_children(node), state),
        }
    }

    fn parameter(&mut self, node: Node<'tree>, state: State) -> State {
        self.record(node, &state);
        let state = self.expand_all(&named_children(node), state);
        let braced = braced_content(node, self.source());
        let modifier = modifier(&braced);
        if !DEFAULT_ASSIGNMENTS
            .iter()
            .any(|prefix| modifier.starts_with(prefix))
        {
            return state;
        }
        let mut assigned = state.clone();
        self.stack
            .write(&mut assigned, &reference(&braced), Variable::UNKNOWN);
        self.stack.merge(vec![assigned, state])
    }

    fn arithmetic(&mut self, nodes: &[Node<'tree>], state: State) -> State {
        nodes
            .iter()
            .fold(state, |state, node| self.arithmetic_node(*node, state))
    }

    fn arithmetic_node(&mut self, node: Node<'tree>, state: State) -> State {
        match node.kind() {
            BINARY if is_assignment_operator(node, self.source()) => {
                let right = node.child_by_field_name(RIGHT_FIELD);
                let state = self.arithmetic(&right.into_iter().collect::<Vec<_>>(), state);
                let target = node.child_by_field_name(LEFT_FIELD);
                self.arithmetic_write(target, state)
            }
            POSTFIX | UNARY if is_step(node, self.source()) => {
                let target = named_children(node).into_iter().next();
                self.arithmetic_write(target, state)
            }
            ASSIGNMENT => {
                let value = node.child_by_field_name(VALUE_FIELD);
                let state = self.arithmetic(&value.into_iter().collect::<Vec<_>>(), state);
                self.arithmetic_write(node.child_by_field_name(NAME_FIELD), state)
            }
            TERNARY => {
                let parts = named_children(node);
                let Some((condition, branches)) = parts.split_first() else {
                    return state;
                };
                let checked = self.arithmetic_node(*condition, state);
                let outcomes = branches
                    .iter()
                    .map(|branch| self.arithmetic_node(*branch, checked.clone()))
                    .collect();
                self.stack.merge(outcomes)
            }
            SIMPLE_EXPANSION | EXPANSION | COMMAND_SUBSTITUTION | PROCESS_SUBSTITUTION
            | ARITHMETIC_EXPANSION => self.expand(node, state),
            _ => self.arithmetic(&named_children(node), state),
        }
    }

    fn arithmetic_write(&mut self, target: Option<Node<'tree>>, state: State) -> State {
        let Some(target) = target else {
            return state;
        };
        let mut state = state;
        let (name, value) = match target.kind() {
            SUBSCRIPT => {
                let index = target.child_by_field_name(INDEX_FIELD);
                state = self.arithmetic(&index.into_iter().collect::<Vec<_>>(), state);
                let name = target
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, self.source()));
                (name, Variable::UNKNOWN)
            }
            VARIABLE_NAME | WORD => (Some(text(target, self.source())), Variable::INTEGER_VALUE),
            _ => (None, Variable::UNKNOWN),
        };
        if let Some(name) = name {
            self.stack.write(&mut state, name, value);
        }
        state
    }
}

fn is_assignment_operator(node: Node<'_>, source: &[u8]) -> bool {
    node.child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| ASSIGNMENT_OPERATORS.contains(&text(operator, source)))
}

fn is_step(node: Node<'_>, source: &[u8]) -> bool {
    node.child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| STEPS.contains(&text(operator, source)))
}

pub fn word_value(interpreter: &Interpreter<'_, '_, '_>, word: Node<'_>, state: &State) -> Variable {
    let source = interpreter.source();
    value_parts(word, source)
        .into_iter()
        .map(|part| {
            let leading = text(part, source)
                .len()
                .saturating_sub(token_text(part, source).len());
            let swallowed = text(part, source).get(..leading).unwrap_or_default();
            Variable::literal(swallowed).append(part_value(interpreter, part, state, source))
        })
        .fold(Variable::UNSET, Variable::append)
}

fn value_parts<'tree>(word: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    match word.kind() {
        CONCATENATION if is_brace_expansion(word, source) => vec![word],
        CONCATENATION | STRING => children(word)
            .into_iter()
            .filter(Node::is_named)
            .flat_map(|part| value_parts(part, source))
            .collect(),
        _ => vec![word],
    }
}

fn part_value(
    interpreter: &Interpreter<'_, '_, '_>,
    part: Node<'_>,
    state: &State,
    source: &[u8],
) -> Variable {
    match part.kind() {
        WORD | NUMBER | RAW_STRING | ANSI_STRING | STRING_CONTENT => {
            literal(part, source).map_or(Variable::UNKNOWN, |text| Variable::literal(&text))
        }
        SIMPLE_EXPANSION | EXPANSION => {
            let braced = braced_content(part, source);
            let name = reference(&braced);
            if name == braced {
                interpreter.stack.lookup(state, &name).0.as_value()
            } else {
                Variable::UNKNOWN
            }
        }
        ARITHMETIC_EXPANSION => Variable::INTEGER_VALUE,
        _ => Variable::UNKNOWN,
    }
}

pub fn is_splitting_word(word: Node<'_>, source: &[u8]) -> bool {
    match word.kind() {
        SIMPLE_EXPANSION | EXPANSION | COMMAND_SUBSTITUTION => true,
        WORD => has_glob(text(word, source)),
        CONCATENATION => {
            is_brace_expansion(word, source)
                || children(word)
                    .into_iter()
                    .any(|part| is_splitting_word(part, source))
        }
        STRING => named_children(word).into_iter().any(|part| {
            EXPANSIONS.contains(&part.kind()) && is_array_expansion(&braced_content(part, source))
        }),
        _ => false,
    }
}

fn has_glob(raw: &str) -> bool {
    let mut escaped = false;
    raw.chars().any(|character| {
        let glob = !escaped && GLOB_CHARACTERS.contains(&character);
        escaped = !escaped && character == ESCAPE;
        glob
    })
}
