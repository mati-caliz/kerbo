use std::cell::RefCell;
use std::collections::BTreeSet;

use super::state::{Reach, Scope, State, Target, Variable};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dependency {
    Variable(String, Option<(Variable, Scope)>),
    Global(String, Option<Variable>),
    Function(String, Option<BTreeSet<Target>>),
    Recursion(usize, bool),
}

#[derive(Debug)]
pub struct Frame {
    pub base: State,
    pub is_function: bool,
    pub entry: usize,
    pub dependencies: RefCell<BTreeSet<Dependency>>,
}

#[derive(Debug, Default)]
pub struct Stack {
    pub frames: Vec<Frame>,
}

impl Frame {
    pub const fn new(base: State, is_function: bool, entry: usize) -> Self {
        Self {
            base,
            is_function,
            entry,
            dependencies: RefCell::new(BTreeSet::new()),
        }
    }
}

impl Stack {
    pub fn lookup(&self, state: &State, name: &str) -> (Variable, Scope) {
        if let Some(found) = state.find(name) {
            return found;
        }
        let found = self.search(|frame| frame.base.find(name));
        self.depend(found.depth, || Dependency::Variable(name.to_owned(), found.value));
        found.value.unwrap_or((Variable::UNKNOWN, Scope::Global))
    }

    pub fn lookup_global(&self, state: &State, name: &str) -> Variable {
        if let Some(found) = state.globals.get(name) {
            return *found;
        }
        let found = self.search(|frame| frame.base.globals.get(name).copied());
        self.depend(found.depth, || Dependency::Global(name.to_owned(), found.value));
        found.value.unwrap_or(Variable::UNKNOWN)
    }

    pub fn lookup_local(&self, state: &State, name: &str) -> Variable {
        let local = |candidate: &State| {
            candidate
                .locals
                .get(name)
                .or_else(|| candidate.prefix.get(name))
                .copied()
        };
        if let Some(found) = local(state) {
            return found;
        }
        for frame in self.frames.iter().rev() {
            if frame.is_function {
                return Variable::UNSET;
            }
            if let Some(found) = local(&frame.base) {
                return found;
            }
        }
        Variable::UNSET
    }

    pub fn function(&self, state: &State, name: &str) -> BTreeSet<Target> {
        if let Some(found) = state.functions.get(name) {
            return found.clone();
        }
        let found = self.search(|frame| frame.base.functions.get(name).cloned());
        let value = found.value.clone();
        self.depend(found.depth, || {
            Dependency::Function(name.to_owned(), value.clone())
        });
        found.value.unwrap_or_else(|| BTreeSet::from([Target::Unknown]))
    }

    pub fn is_on_stack(&self, entry: usize) -> bool {
        let found = self.frames.iter().any(|frame| frame.entry == entry);
        self.depend(self.frames.len(), || Dependency::Recursion(entry, found));
        found
    }

    fn search<Found>(&self, find: impl Fn(&Frame) -> Option<Found>) -> Searched<Found> {
        for (offset, frame) in self.frames.iter().rev().enumerate() {
            if let Some(value) = find(frame) {
                return Searched {
                    value: Some(value),
                    depth: offset.saturating_add(1),
                };
            }
        }
        Searched {
            value: None,
            depth: self.frames.len(),
        }
    }

    fn depend(&self, depth: usize, dependency: impl Fn() -> Dependency) {
        for frame in self.frames.iter().rev().take(depth) {
            frame.dependencies.borrow_mut().insert(dependency());
        }
    }

    pub fn holds(&self, caller: &State, entry: usize, dependency: &Dependency) -> bool {
        match dependency {
            Dependency::Variable(name, expected) => {
                let found = caller
                    .find(name)
                    .or_else(|| self.frames.iter().rev().find_map(|frame| frame.base.find(name)));
                found == *expected
            }
            Dependency::Global(name, expected) => {
                let found = caller.globals.get(name).copied().or_else(|| {
                    self.frames
                        .iter()
                        .rev()
                        .find_map(|frame| frame.base.globals.get(name).copied())
                });
                found == *expected
            }
            Dependency::Function(name, expected) => {
                let found = caller.functions.get(name).cloned().or_else(|| {
                    self.frames
                        .iter()
                        .rev()
                        .find_map(|frame| frame.base.functions.get(name).cloned())
                });
                found == *expected
            }
            Dependency::Recursion(recursive, expected) => {
                let found = *recursive == entry || self.frames.iter().any(|frame| frame.entry == *recursive);
                found == *expected
            }
        }
    }

    pub fn replay(&self, caller: &State, dependency: &Dependency) {
        match dependency {
            Dependency::Variable(name, _) => {
                self.lookup(caller, name);
            }
            Dependency::Global(name, _) => {
                self.lookup_global(caller, name);
            }
            Dependency::Function(name, _) => {
                self.function(caller, name);
            }
            Dependency::Recursion(entry, _) => {
                self.is_on_stack(*entry);
            }
        }
    }

    pub fn write(&self, state: &mut State, name: &str, value: Variable) {
        let (existing, _) = self.lookup(state, name);
        self.put(state, name, existing.with_value(value));
    }

    pub fn put(&self, state: &mut State, name: &str, variable: Variable) {
        let (_, scope) = self.lookup(state, name);
        let scope = if scope == Scope::Prefix {
            Scope::Local
        } else {
            scope
        };
        state.insert(scope, name, variable);
    }

    pub fn write_global(&self, state: &mut State, name: &str, value: Variable) {
        let existing = self.lookup_global(state, name);
        state.insert(Scope::Global, name, existing.with_value(value));
    }

    pub fn write_local(&self, state: &mut State, name: &str, value: Variable) {
        let existing = self.lookup_local(state, name);
        state.insert(Scope::Local, name, existing.with_value(value));
    }

    pub fn merge(&self, states: Vec<State>) -> State {
        let unreachable = states.iter().any(|state| state.reach == Reach::Unreachable);
        let mut live = states.into_iter().filter(State::is_live);
        let Some(first) = live.next() else {
            return State::empty(if unreachable {
                Reach::Unreachable
            } else {
                Reach::Dead
            });
        };
        live.fold(first, |merged, other| {
            super::merging::merge_pair(self, merged, &other)
        })
    }
}

struct Searched<Found> {
    value: Option<Found>,
    depth: usize,
}
