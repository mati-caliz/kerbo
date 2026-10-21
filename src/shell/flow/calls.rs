use std::collections::BTreeSet;

use super::Interpreter;
use super::frames::Frame;
use super::state::{Reach, State, Target};
use crate::shell::syntax::BODY_FIELD;

const CACHED_RESULTS: usize = 10;

pub trait Calls {
    fn call(&mut self, name: &str, state: State) -> State;
    fn invoke(&mut self, entry: usize, caller: &State) -> State;
    fn cached(&self, entry: usize, caller: &State) -> Option<State>;
    fn run_stragglers(&mut self, environment: &State, exit: &State);
}

impl Calls for Interpreter<'_, '_, '_> {
    fn call(&mut self, name: &str, state: State) -> State {
        let targets = self.stack.function(&state, name);
        let mut branches = Vec::new();
        for target in targets {
            let Target::Defined(entry) = target else {
                branches.push(state.clone());
                continue;
            };
            if self.stack.is_on_stack(entry) {
                branches.push(state.clone());
                continue;
            }
            let diff = self.invoke(entry, &state);
            let mut result = state.clone();
            if diff.is_live() {
                result.patch(&diff);
            } else {
                result = State::empty(Reach::Unreachable);
            }
            branches.push(result);
        }
        if branches.len() == 1 {
            return branches.pop().unwrap_or(state);
        }
        let merged = self.stack.merge(branches);
        if !merged.is_live() {
            return merged;
        }
        let mut patched = state;
        patched.patch(&merged);
        patched
    }

    fn invoke(&mut self, entry: usize, caller: &State) -> State {
        if let Some(cached) = self.cached(entry, caller) {
            return cached;
        }
        let Some(body) = self
            .functions
            .get(&entry)
            .and_then(|definition| definition.child_by_field_name(BODY_FIELD))
        else {
            return State::default();
        };
        self.invoked.insert(entry);
        self.stack.frames.push(Frame::new(caller.clone(), true, entry));
        let was_in_function = std::mem::replace(&mut self.in_function, true);
        let mut exit = self.run_frame(body, State::default());
        self.in_function = was_in_function;
        let dependencies = self
            .stack
            .frames
            .pop()
            .map(|frame| frame.dependencies.into_inner())
            .unwrap_or_default();
        exit.clear_locals();
        if exit.reach == Reach::Dead {
            exit = State::empty(Reach::Unreachable);
        }
        let entries = self.memo.entry(entry).or_default();
        entries.insert(0, (dependencies.into_iter().collect(), exit.clone()));
        entries.truncate(CACHED_RESULTS);
        exit
    }

    fn cached(&self, entry: usize, caller: &State) -> Option<State> {
        let entries = self.memo.get(&entry)?;
        let (dependencies, result) = entries.iter().find(|(dependencies, _)| {
            dependencies
                .iter()
                .all(|dependency| self.stack.holds(caller, entry, dependency))
        })?;
        for dependency in dependencies {
            self.stack.replay(caller, dependency);
        }
        Some(result.clone())
    }

    fn run_stragglers(&mut self, environment: &State, exit: &State) {
        if !exit.is_live() {
            return;
        }
        let mut input = environment.clone();
        input.patch(exit);
        let declared: BTreeSet<usize> = exit
            .functions
            .values()
            .flatten()
            .filter_map(|target| match target {
                Target::Defined(entry) => Some(*entry),
                Target::Unknown => None,
            })
            .collect();
        let uninvoked: Vec<usize> = declared
            .into_iter()
            .filter(|entry| !self.invoked.contains(entry))
            .collect();
        for entry in uninvoked {
            self.invoke(entry, &input);
        }
    }
}
