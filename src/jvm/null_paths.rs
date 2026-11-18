use std::collections::{HashMap, HashSet};

use super::instructions::{Body, Operation};
use super::null_frames::{Frame, Slot, execute};
use super::null_partial::{Partial, join, meet};

// El `STEPS_LIMIT` de IntelliJ: un método que pide más pasos que esto queda sin inferir.
const STEPS_LIMIT: usize = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct State {
    instruction: usize,
    frame: Frame,
    null_checked: bool,
}

#[derive(Debug)]
enum Step {
    Proceed(usize),
    Combine {
        state: usize,
        delta: Partial,
        successors: Vec<usize>,
    },
}

enum Next {
    Settled(Partial),
    Returned,
    OnlyWhenNull(usize),
    Successors,
}

// Calca `NonNullInAnalysis` de IntelliJ: recorre los caminos del método con el parámetro en null
// (en un `if (p == null)` sigue sólo esa rama) y junta qué le pasa en cada uno. Un estado que se
// repite mientras se lo está recorriendo es una vuelta de bucle y no aporta nada.
struct Exploration<'body> {
    body: &'body Body,
    states: Vec<State>,
    below_a_call: Vec<bool>,
    results: Vec<Option<Partial>>,
    pending: Vec<Step>,
    computed: HashMap<State, usize>,
    open: HashSet<State>,
    returned_early: bool,
}

pub(super) fn is_branching(body: &Body) -> bool {
    !body.handlers.is_empty()
        || body.operations.iter().any(|operation| {
            matches!(
                operation,
                Operation::NullBranch { .. }
                    | Operation::ZeroBranch { .. }
                    | Operation::Branch { .. }
                    | Operation::Jump(_)
                    | Operation::Switch(_)
            )
        })
}

// Un método sin ramas (el `CombinedAnalysis` de IntelliJ): alcanza con que lo desreferencie o con
// que alguna de las llamadas a las que lo pasa lo exija no nulo.
pub(super) fn straight(body: &Body, parameter_local: usize) -> Option<Partial> {
    let mut frame = Frame::entry(body.max_locals, parameter_local);
    let mut result = Partial::Identity;
    for operation in &body.operations {
        match execute(&mut frame, operation)? {
            Partial::Dereference => return Some(Partial::Dereference),
            Partial::Conditional(products) => {
                result = meet(result, Partial::Conditional(products)).ok()?;
            }
            Partial::Identity | Partial::Return => {}
        }
    }
    Some(result)
}

pub(super) fn explore(body: &Body, parameter_local: usize) -> Option<Partial> {
    let mut exploration = Exploration {
        body,
        states: Vec::new(),
        below_a_call: Vec::new(),
        results: Vec::new(),
        pending: Vec::new(),
        computed: HashMap::new(),
        open: HashSet::new(),
        returned_early: false,
    };
    let start = State {
        instruction: 0,
        frame: Frame::entry(body.max_locals, parameter_local),
        null_checked: false,
    };
    let first = exploration.add(start, false);
    exploration.pending.push(Step::Proceed(first));
    let mut steps = 0_usize;
    while let Some(step) = exploration.pending.pop() {
        steps += 1;
        if steps > STEPS_LIMIT {
            return None;
        }
        match step {
            Step::Proceed(state) => exploration.proceed(state)?,
            Step::Combine {
                state,
                delta,
                successors,
            } => exploration.combine(state, delta, &successors)?,
        }
        if exploration.returned_early {
            return Some(Partial::Return);
        }
    }
    exploration.results.first().cloned().flatten()
}

impl Exploration<'_> {
    fn add(&mut self, state: State, below_a_call: bool) -> usize {
        self.states.push(state);
        self.below_a_call.push(below_a_call);
        self.results.push(None);
        self.states.len() - 1
    }

    fn settle(&mut self, state: usize, result: Partial) -> Option<()> {
        *self.results.get_mut(state)? = Some(result);
        Some(())
    }

    fn finish(&mut self, index: usize, state: State, result: Partial) -> Option<()> {
        self.computed.insert(state, index);
        self.settle(index, result)
    }

    fn proceed(&mut self, index: usize) -> Option<()> {
        let state = self.states.get(index)?.clone();
        if self.open.contains(&state) {
            return self.settle(index, Partial::Identity);
        }
        if let Some(known) = self.computed.get(&state) {
            let result = self.results.get(*known)?.clone()?;
            return self.settle(index, result);
        }
        let operation = self.body.operations.get(state.instruction)?;
        let mut after = state.frame.clone();
        let delta = execute(&mut after, operation)?;
        if delta == Partial::Dereference {
            return self.finish(index, state, Partial::Dereference);
        }
        let below_a_call = *self.below_a_call.get(index)?;
        match next(operation, &state) {
            Next::Settled(result) => self.finish(index, state, result),
            Next::Returned if below_a_call => self.finish(index, state, Partial::Return),
            Next::Returned => {
                self.returned_early = true;
                Some(())
            }
            Next::OnlyWhenNull(instruction) => {
                let followed = State {
                    instruction,
                    frame: after,
                    null_checked: true,
                };
                self.branch(index, delta, vec![followed]);
                Some(())
            }
            Next::Successors => {
                let followed = self.successors(operation, &state, &after);
                self.branch(index, delta, followed);
                Some(())
            }
        }
    }

    fn branch(&mut self, index: usize, delta: Partial, followed: Vec<State>) {
        let below_a_call =
            self.below_a_call.get(index).copied().unwrap_or_default() || delta != Partial::Identity;
        if let Some(state) = self.states.get(index) {
            self.open.insert(state.clone());
        }
        let successors: Vec<usize> = followed
            .into_iter()
            .map(|successor| self.add(successor, below_a_call))
            .collect();
        self.pending.push(Step::Combine {
            state: index,
            delta,
            successors: successors.clone(),
        });
        self.pending.extend(successors.into_iter().map(Step::Proceed));
    }

    fn successors(&self, operation: &Operation, state: &State, after: &Frame) -> Vec<State> {
        let next_instruction = state.instruction + 1;
        let mut instructions = match operation {
            Operation::Jump(target) => vec![*target],
            Operation::Switch(targets) => targets.clone(),
            Operation::NullBranch { target, .. }
            | Operation::ZeroBranch { target, .. }
            | Operation::Branch { target, .. } => vec![next_instruction, *target],
            Operation::Effect { .. }
            | Operation::LoadReference(_)
            | Operation::StoreReference(_)
            | Operation::StorePrimitive { .. }
            | Operation::Duplicate { .. }
            | Operation::Swap
            | Operation::Dereference { .. }
            | Operation::TypeCheck
            | Operation::Invoke(_)
            | Operation::Return
            | Operation::Throw => vec![next_instruction],
        };
        instructions.dedup();
        let mut found: Vec<State> = instructions
            .iter()
            .map(|instruction| State {
                instruction: *instruction,
                frame: after.clone(),
                null_checked: state.null_checked,
            })
            .collect();
        let handlers = self.body.handlers.iter().filter(|handler| {
            (handler.start..handler.end).contains(&state.instruction)
                && !instructions.contains(&handler.target)
        });
        for handler in handlers {
            let caught = State {
                instruction: handler.target,
                frame: state.frame.caught(),
                null_checked: state.null_checked,
            };
            if !found.contains(&caught) {
                found.push(caught);
            }
        }
        found
    }

    fn combine(&mut self, index: usize, delta: Partial, successors: &[usize]) -> Option<()> {
        let mut joined = Partial::Identity;
        for successor in successors {
            joined = join(joined, self.results.get(*successor)?.clone()?).ok()?;
        }
        let state = self.states.get(index)?.clone();
        self.open.remove(&state);
        self.finish(index, state, meet(delta, joined).ok()?)
    }
}

fn next(operation: &Operation, state: &State) -> Next {
    let top = state.frame.top();
    let following = state.instruction + 1;
    match operation {
        Operation::Return => Next::Returned,
        Operation::Throw if state.null_checked => Next::Settled(Partial::Dereference),
        Operation::Throw => Next::Settled(Partial::Identity),
        Operation::NullBranch {
            jumps_on_null,
            target,
        } if top == Some(Slot::Parameter) => {
            Next::OnlyWhenNull(if *jumps_on_null { *target } else { following })
        }
        Operation::ZeroBranch {
            jumps_on_zero,
            target,
        } if top == Some(Slot::TypeCheck) => {
            Next::OnlyWhenNull(if *jumps_on_zero { *target } else { following })
        }
        Operation::NullBranch { .. }
        | Operation::ZeroBranch { .. }
        | Operation::Effect { .. }
        | Operation::LoadReference(_)
        | Operation::StoreReference(_)
        | Operation::StorePrimitive { .. }
        | Operation::Duplicate { .. }
        | Operation::Swap
        | Operation::Dereference { .. }
        | Operation::TypeCheck
        | Operation::Invoke(_)
        | Operation::Branch { .. }
        | Operation::Jump(_)
        | Operation::Switch(_) => Next::Successors,
    }
}
