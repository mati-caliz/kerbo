use std::rc::Rc;

use super::instructions::{Call, CallKind, Operation};
use super::null_partial::{Partial, PassedParameter, meet, passed};

// Lo que hay en un lugar de la pila o de las locales: el parámetro que se analiza, el resultado de
// un `instanceof` sobre él, o cualquier otra cosa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Slot {
    Parameter,
    TypeCheck,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Frame {
    pub(super) locals: Vec<Slot>,
    pub(super) stack: Vec<Slot>,
}

impl Frame {
    pub(super) fn entry(max_locals: usize, parameter_local: usize) -> Self {
        let mut locals = vec![Slot::Other; max_locals];
        if let Some(local) = locals.get_mut(parameter_local) {
            *local = Slot::Parameter;
        }
        Self {
            locals,
            stack: Vec::new(),
        }
    }

    pub(super) fn top(&self) -> Option<Slot> {
        self.stack.last().copied()
    }

    // El marco con el que arranca un `catch`: las locales de antes de la instrucción y sólo la
    // excepción en la pila.
    pub(super) fn caught(&self) -> Self {
        Self {
            locals: self.locals.clone(),
            stack: vec![Slot::Other],
        }
    }

    fn pop(&mut self, count: usize) -> Option<Vec<Slot>> {
        let kept = self.stack.len().checked_sub(count)?;
        Some(self.stack.split_off(kept))
    }

    fn push_other(&mut self, count: usize) {
        self.stack.extend(std::iter::repeat_n(Slot::Other, count));
    }

    fn duplicate(&mut self, count: usize, under: usize) -> Option<()> {
        let copied_from = self.stack.len().checked_sub(count)?;
        let inserted_at = copied_from.checked_sub(under)?;
        let copied = self.stack.get(copied_from..)?.to_vec();
        self.stack.splice(inserted_at..inserted_at, copied);
        Some(())
    }

    fn dereference(&mut self, depth: usize, pops: usize, pushes: usize) -> Option<Partial> {
        let position = self.stack.len().checked_sub(depth + 1)?;
        let target = *self.stack.get(position)?;
        self.pop(pops)?;
        self.push_other(pushes);
        Some(if target == Slot::Parameter {
            Partial::Dereference
        } else {
            Partial::Identity
        })
    }

    fn store(&mut self, local: usize, width: usize) -> Option<()> {
        self.pop(width)?;
        for offset in 0..width {
            *self.locals.get_mut(local + offset)? = Slot::Other;
        }
        Some(())
    }
}

// Ejecuta la instrucción sobre el marco y dice qué le hizo al parámetro. `None` si la pila no da.
pub(super) fn execute(frame: &mut Frame, operation: &Operation) -> Option<Partial> {
    match operation {
        Operation::Effect { pops, pushes } => {
            frame.pop(*pops)?;
            frame.push_other(*pushes);
        }
        Operation::LoadReference(local) => frame.stack.push(*frame.locals.get(*local)?),
        Operation::StoreReference(local) => *frame.locals.get_mut(*local)? = frame.stack.pop()?,
        Operation::StorePrimitive { local, width } => frame.store(*local, *width)?,
        Operation::Duplicate { count, under } => frame.duplicate(*count, *under)?,
        Operation::Swap => {
            let top = frame.stack.pop()?;
            let below = frame.stack.pop()?;
            frame.stack.extend([top, below]);
        }
        Operation::Dereference { depth, pops, pushes } => return frame.dereference(*depth, *pops, *pushes),
        Operation::TypeCheck => {
            let checked = frame.stack.pop()? == Slot::Parameter;
            frame
                .stack
                .push(if checked { Slot::TypeCheck } else { Slot::Other });
        }
        Operation::Invoke(call) => return invoke(frame, call),
        Operation::NullBranch { .. } | Operation::ZeroBranch { .. } | Operation::Switch(_) => {
            frame.stack.pop()?;
        }
        Operation::Branch { pops, .. } => {
            frame.pop(*pops)?;
        }
        Operation::Jump(_) | Operation::Return | Operation::Throw => {}
    }
    Some(Partial::Identity)
}

fn invoke(frame: &mut Frame, call: &Call) -> Option<Partial> {
    let mut passed_at = Vec::new();
    for (index, argument) in call.arguments.iter().enumerate().rev() {
        if frame.pop(argument.width)?.first() == Some(&Slot::Parameter) {
            passed_at.push(index);
        }
    }
    let has_receiver = !matches!(call.kind, CallKind::Static | CallKind::Dynamic);
    let dereferenced = has_receiver && frame.stack.pop()? == Slot::Parameter;
    frame.push_other(call.returned);
    if dereferenced {
        return Some(Partial::Dereference);
    }
    let stable_call = match call.kind {
        CallKind::Static | CallKind::Special => true,
        CallKind::Virtual => false,
        CallKind::Interface | CallKind::Dynamic => return Some(Partial::Identity),
    };
    passed_at
        .into_iter()
        .rev()
        .try_fold(Partial::Identity, |delta, index| {
            let parameter = PassedParameter {
                target: Rc::clone(&call.target),
                index,
                stable_call,
            };
            meet(delta, passed(parameter)).ok()
        })
}
