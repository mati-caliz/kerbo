use tree_sitter::Node;

use super::AssignmentScope;
use crate::shell::commands::generic_options;
use crate::shell::flow::Interpreter;
use crate::shell::flow::commands::Commands;
use crate::shell::flow::state::{Scope, State, Variable};
use crate::shell::flow::values::Values;
use crate::shell::parameters::is_variable_name;
use crate::shell::syntax::{
    ARRAY, ASSIGNMENT, INDEX_FIELD, NAME_FIELD, SUBSCRIPT, VALUE_FIELD, VARIABLE_NAME, children, text,
};
use crate::shell::words::literal_or;

const INTEGER_FLAG: &str = "i";
const GLOBAL_FLAG: &str = "g";
const PROPERTY_FLAGS: &[&str] = &["a", "A", "i", "x"];
const UNSET_PREFIX: char = '+';
const INTEGER_LETTER: char = 'i';
const NOT_LITERAL: &str = "\0";
const NUL: char = '\0';
const ASSIGNMENT_SIGN: char = '=';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Properties {
    None,
    Other,
    Integer,
}

#[derive(Debug, Clone, Copy)]
pub struct Declaring {
    scope: Option<Scope>,
    writer: AssignmentScope,
    properties: Properties,
    removes_integer: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Change {
    scope: Option<Scope>,
    integer: Option<bool>,
}

pub trait Declarations<'tree> {
    fn declaration(&mut self, arguments: &[Node<'tree>], state: State) -> State;
    fn declaring(&self, arguments: &[Node<'_>]) -> Declaring;
    fn evaluate_declared(&mut self, argument: Node<'tree>, state: State) -> State;
    fn declare_value(&self, argument: Node<'_>, declaring: Declaring, state: &mut State);
    fn declared_name(&self, argument: Node<'_>) -> Option<(String, bool)>;
    fn change_properties(&self, argument: Node<'_>, declaring: Declaring, state: &mut State);
    fn change_integer(&self, state: &mut State, name: &str, change: Change);
}

impl<'tree> Declarations<'tree> for Interpreter<'tree, '_, '_> {
    fn declaration(&mut self, arguments: &[Node<'tree>], state: State) -> State {
        let declaring = self.declaring(arguments);
        let mut state = arguments
            .iter()
            .fold(state, |state, argument| self.evaluate_declared(*argument, state));
        for argument in arguments {
            self.declare_value(*argument, declaring, &mut state);
        }
        for argument in arguments {
            self.change_properties(*argument, declaring, &mut state);
        }
        for argument in arguments {
            let Some((name, plain)) = self.declared_name(*argument) else {
                continue;
            };
            let removed = declaring.removes_integer && (plain || declaring.properties == Properties::Integer);
            if plain || declaring.removes_integer {
                let change = Change {
                    scope: declaring.scope,
                    integer: removed.then_some(false),
                };
                self.change_integer(&mut state, &name, change);
            }
        }
        state
    }

    fn declaring(&self, arguments: &[Node<'_>]) -> Declaring {
        let source = self.source();
        let flags: Vec<String> = generic_options(arguments, source)
            .into_iter()
            .map(|(flag, _)| flag)
            .collect();
        let has = |wanted: &str| flags.iter().any(|flag| flag == wanted);
        let removes_integer = arguments.iter().any(|argument| {
            literal_or(*argument, source, NOT_LITERAL)
                .strip_prefix(UNSET_PREFIX)
                .is_some_and(|letters| letters.contains(INTEGER_LETTER))
        });
        let (scope, writer) = if has(GLOBAL_FLAG) {
            (Some(Scope::Global), AssignmentScope::Global)
        } else if self.in_function {
            (Some(Scope::Local), AssignmentScope::Local)
        } else {
            (None, AssignmentScope::Variable)
        };
        let properties = if has(INTEGER_FLAG) {
            Properties::Integer
        } else if PROPERTY_FLAGS.iter().any(|flag| has(flag)) {
            Properties::Other
        } else {
            Properties::None
        };
        Declaring {
            scope,
            writer,
            properties,
            removes_integer,
        }
    }

    fn evaluate_declared(&mut self, argument: Node<'tree>, state: State) -> State {
        if argument.kind() != ASSIGNMENT {
            return self.expand(argument, state);
        }
        let mut state = state;
        if let Some(target) = argument
            .child_by_field_name(NAME_FIELD)
            .filter(|target| target.kind() == SUBSCRIPT)
        {
            let index = target.child_by_field_name(INDEX_FIELD);
            state = self.arithmetic(&index.into_iter().collect::<Vec<_>>(), state);
        }
        match argument.child_by_field_name(VALUE_FIELD) {
            Some(value) if value.kind() == ARRAY => self.expand_all(&children(value), state),
            Some(value) => self.expand(value, state),
            None => state,
        }
    }

    fn declare_value(&self, argument: Node<'_>, declaring: Declaring, state: &mut State) {
        if argument.kind() == ASSIGNMENT {
            let Some(target) = argument.child_by_field_name(NAME_FIELD) else {
                return;
            };
            let indexed = target.kind() == SUBSCRIPT;
            let name_node = if indexed {
                target.child_by_field_name(NAME_FIELD)
            } else {
                Some(target)
            };
            let Some(name) = name_node.map(|node| text(node, self.source())) else {
                return;
            };
            let array = argument
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| value.kind() == ARRAY);
            let computed = if indexed || array {
                Variable::UNKNOWN
            } else {
                self.assigned_value(argument, name, state)
            };
            self.store(state, (name, computed), declaring.writer);
            return;
        }
        let raw = literal_or(argument, self.source(), NOT_LITERAL);
        let Some((name, value)) = raw.split_once(ASSIGNMENT_SIGN) else {
            return;
        };
        if !is_variable_name(name) {
            return;
        }
        let computed = if raw.contains(NUL) {
            Variable::UNKNOWN
        } else {
            Variable::literal(value)
        };
        self.store(state, (name, computed), declaring.writer);
    }

    fn declared_name(&self, argument: Node<'_>) -> Option<(String, bool)> {
        if argument.kind() == ASSIGNMENT {
            let target = argument.child_by_field_name(NAME_FIELD)?;
            let name = if target.kind() == SUBSCRIPT {
                target.child_by_field_name(NAME_FIELD)?
            } else {
                target
            };
            return Some((text(name, self.source()).to_owned(), false));
        }
        let raw = if argument.kind() == VARIABLE_NAME {
            text(argument, self.source()).to_owned()
        } else {
            literal_or(argument, self.source(), NOT_LITERAL)
        };
        is_variable_name(&raw).then_some((raw, true))
    }

    fn change_properties(&self, argument: Node<'_>, declaring: Declaring, state: &mut State) {
        let Some((name, plain)) = self.declared_name(argument) else {
            return;
        };
        if plain || declaring.properties != Properties::None {
            let change = Change {
                scope: declaring.scope,
                integer: (declaring.properties == Properties::Integer).then_some(true),
            };
            self.change_integer(state, &name, change);
        }
    }

    fn change_integer(&self, state: &mut State, name: &str, change: Change) {
        let current = match change.scope {
            None => self.stack.lookup(state, name).0,
            Some(Scope::Global) => self.stack.lookup_global(state, name),
            Some(Scope::Local | Scope::Prefix) => self.stack.lookup_local(state, name),
        };
        let changed = Variable {
            space: current.space,
            integer: change.integer.unwrap_or(current.integer),
        };
        match change.scope {
            None => self.stack.put(state, name, changed),
            Some(Scope::Global) => state.insert(Scope::Global, name, changed),
            Some(Scope::Local | Scope::Prefix) => state.insert(Scope::Local, name, changed),
        }
    }
}
