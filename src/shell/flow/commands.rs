pub mod builtins;
pub mod declarations;

use tree_sitter::Node;

use super::Interpreter;
use super::state::{Reach, Scope, State, Variable};
use super::values::word_value;
use crate::shell::commands::Command;
use crate::shell::flow::calls::Calls;
use crate::shell::flow::commands::builtins::Builtins;
use crate::shell::flow::commands::declarations::Declarations;
use crate::shell::flow::values::Values;
use crate::shell::syntax::{
    ARRAY, ASSIGNMENT, COMMAND, DECLARATION, INDEX_FIELD, NAME_FIELD, REDIRECT_FIELD, SUBSCRIPT, UNSET,
    VALUE_FIELD, WORD, children, field_children, text,
};
use crate::shell::words::literal;

const APPEND: &str = "+=";
const BUILTIN: &str = "builtin";
const COMMAND_BUILTIN: &str = "command";
const DECLARING: &[&str] = &["declare", "local", "typeset"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentScope {
    Variable,
    Prefix,
    Local,
    Global,
}

#[derive(Debug, Clone, Copy)]
pub struct Invocation<'list, 'tree> {
    pub words: &'list [Node<'tree>],
    pub prefix: &'list [Node<'tree>],
}

impl<'list, 'tree> Invocation<'list, 'tree> {
    const fn bare(words: &'list [Node<'tree>]) -> Self {
        Self { words, prefix: &[] }
    }

    fn rest(self) -> Self {
        Self {
            words: self.words.get(1..).unwrap_or_default(),
            prefix: self.prefix,
        }
    }
}

pub trait Commands<'tree> {
    fn command(&mut self, node: Node<'tree>, state: State) -> State;
    fn dispatch(&mut self, invocation: Invocation<'_, 'tree>, state: State, name: Option<String>) -> State;
    fn expand_words(&mut self, invocation: Invocation<'_, 'tree>, state: State) -> State;
    fn regular(&mut self, invocation: Invocation<'_, 'tree>, state: State, called: Option<String>) -> State;
    fn assignment(&mut self, node: Node<'tree>, state: State, scope: AssignmentScope) -> State;
    fn assigned_value(&self, node: Node<'_>, name: &str, state: &State) -> Variable;
    fn store(&self, state: &mut State, assigned: (&str, Variable), scope: AssignmentScope);
    fn finish_frame(&mut self, state: State) -> State;
}

impl<'tree> Commands<'tree> for Interpreter<'tree, '_, '_> {
    fn command(&mut self, node: Node<'tree>, state: State) -> State {
        let Some(command) = Command::of(node) else {
            return state;
        };
        let state = if node.kind() == COMMAND {
            self.expand_all(&field_children(node, REDIRECT_FIELD), state)
        } else {
            state
        };
        match node.kind() {
            DECLARATION => {
                let keyword = command.first_word().map_or("", |word| text(word, self.source()));
                if DECLARING.contains(&keyword) {
                    self.declaration(command.arguments(), state)
                } else {
                    self.regular(Invocation::bare(&command.words), state, None)
                }
            }
            UNSET => {
                let expanded = self.regular(Invocation::bare(&command.words), state, None);
                self.unset(command.arguments(), expanded)
            }
            _ => {
                let name = command
                    .first_word()
                    .filter(|word| word.kind() == WORD)
                    .and_then(|word| literal(word, self.source()));
                let invocation = Invocation {
                    words: &command.words,
                    prefix: &command.assignments,
                };
                self.dispatch(invocation, state, name)
            }
        }
    }

    fn dispatch(&mut self, invocation: Invocation<'_, 'tree>, state: State, name: Option<String>) -> State {
        let Some(name) = name else {
            return self.regular(invocation, state, None);
        };
        if let Some(handled) = self.builtin(&name, invocation, &state) {
            return handled;
        }
        let rest = invocation.rest();
        let next = rest.words.first().and_then(|word| literal(*word, self.source()));
        match name.as_str() {
            BUILTIN if !rest.words.is_empty() => self.dispatch(rest, state, next),
            COMMAND_BUILTIN if !rest.words.is_empty() => self.regular(rest, state, next),
            declaring if DECLARING.contains(&declaring) => self.declaration(rest.words, state),
            _ => self.regular(invocation, state, Some(name)),
        }
    }

    fn expand_words(&mut self, invocation: Invocation<'_, 'tree>, state: State) -> State {
        let state = invocation.words.iter().fold(state, |state, word| {
            if word.kind() == ASSIGNMENT {
                self.assignment(*word, state, AssignmentScope::Variable)
            } else {
                self.expand(*word, state)
            }
        });
        invocation.prefix.iter().fold(state, |state, assignment| {
            self.assignment(*assignment, state, AssignmentScope::Prefix)
        })
    }

    fn regular(&mut self, invocation: Invocation<'_, 'tree>, state: State, called: Option<String>) -> State {
        let expanded = self.expand_words(invocation, state);
        let mut finished = match called {
            Some(name) => self.call(&name, expanded),
            None => expanded,
        };
        if !invocation.prefix.is_empty() {
            finished.clear_prefix();
        }
        finished
    }

    fn assignment(&mut self, node: Node<'tree>, state: State, scope: AssignmentScope) -> State {
        let value = node.child_by_field_name(VALUE_FIELD);
        let mut state = match value {
            Some(value) if value.kind() == ARRAY => self.expand_all(&children(value), state),
            Some(value) => self.expand(value, state),
            None => state,
        };
        let Some(target) = node.child_by_field_name(NAME_FIELD) else {
            return state;
        };
        let indexed = target.kind() == SUBSCRIPT;
        let name_node = if indexed {
            let index = target.child_by_field_name(INDEX_FIELD);
            state = self.arithmetic(&index.into_iter().collect::<Vec<_>>(), state);
            target.child_by_field_name(NAME_FIELD)
        } else {
            Some(target)
        };
        let Some(name) = name_node.map(|name| text(name, self.source())) else {
            return state;
        };
        let computed = if indexed || value.is_some_and(|value| value.kind() == ARRAY) {
            Variable::UNKNOWN
        } else {
            self.assigned_value(node, name, &state)
        };
        self.store(&mut state, (name, computed), scope);
        state
    }

    fn assigned_value(&self, node: Node<'_>, name: &str, state: &State) -> Variable {
        let appends = children(node)
            .into_iter()
            .any(|child| !child.is_named() && text(child, self.source()) == APPEND);
        let start = if appends {
            self.stack.lookup(state, name).0.as_value()
        } else {
            Variable::UNSET
        };
        node.child_by_field_name(VALUE_FIELD)
            .map_or(start, |value| start.append(word_value(self, value, state)))
    }

    fn store(&self, state: &mut State, (name, value): (&str, Variable), scope: AssignmentScope) {
        match scope {
            AssignmentScope::Variable => self.stack.write(state, name, value),
            AssignmentScope::Local => self.stack.write_local(state, name, value),
            AssignmentScope::Global => self.stack.write_global(state, name, value),
            AssignmentScope::Prefix => state.insert(Scope::Prefix, name, value.as_value()),
        }
    }

    fn finish_frame(&mut self, state: State) -> State {
        if let Some(returns) = self.returns.last_mut() {
            returns.push(state);
        }
        State::empty(Reach::Dead)
    }
}
