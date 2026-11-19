use std::collections::HashSet;

use super::instructions::{Body, Mark, Operation, decode};
use super::library::Library;
use super::method::Method;
use super::null_frames::{Frame, Slot, execute};
use super::null_parameters::parameter_local;
use super::null_partial::Partial;

// Cuántos estados (instrucción y marco) se recorren antes de dar el método por no inferido.
const STATE_LIMIT: usize = 4096;

// Si IntelliJ infiere del bytecode el contrato `null -> null` para ese parámetro: con null ahí, todo
// camino que devuelve algo devuelve null (el `InOutAnalysis` con el parámetro en null). Sólo en un
// método que nadie puede redefinir, y sin `try`: lo que pasa en un `catch` no se sigue.
pub fn returns_null_for_null(library: &Library, class: &str, method: &Method, index: usize) -> bool {
    library.code_of(class).is_some_and(|code| {
        code.method(&method.name, &method.descriptor)
            .filter(|declared| code.is_stable(declared))
            .and_then(|declared| {
                let local = parameter_local(declared, index)?;
                let body = decode(&code.pool, declared.code.as_ref()?)?;
                Some(body.handlers.is_empty() && only_returns_null(&body, local))
            })
            .unwrap_or(false)
    })
}

// Recorre el método con el parámetro en null. En los marcos, `Slot::Parameter` es cualquier valor
// que se sabe null: el parámetro o un `null` escrito.
fn only_returns_null(body: &Body, parameter_local: usize) -> bool {
    let mut pending = vec![(0, Frame::entry(body.max_locals, parameter_local))];
    let mut seen = HashSet::new();
    let mut returned_null = false;
    while let Some((position, mut frame)) = pending.pop() {
        if !seen.insert((position, frame.clone())) {
            continue;
        }
        let (Some(operation), Some(mark)) = (body.operations.get(position), body.marks.get(position)) else {
            return false;
        };
        let top = frame.top();
        if seen.len() > STATE_LIMIT || (*operation == Operation::Return && top != Some(Slot::Parameter)) {
            return false;
        }
        if *mark == Mark::ReferenceReturn {
            returned_null = true;
            continue;
        }
        match execute(&mut frame, operation) {
            None => return false,
            Some(Partial::Dereference) => continue,
            Some(Partial::Identity | Partial::Return | Partial::Conditional(_)) => {}
        }
        if let (Mark::NullConstant, Some(pushed)) = (*mark, frame.stack.last_mut()) {
            *pushed = Slot::Parameter;
        }
        let reached = successors(operation, top, position + 1);
        pending.extend(reached.into_iter().map(|target| (target, frame.clone())));
    }
    returned_null
}

// A dónde sigue la ejecución: un salto sobre algo que se sabe null (o sobre el `instanceof` de un
// null, que da cero) toma un solo camino.
fn successors(operation: &Operation, top: Option<Slot>, next: usize) -> Vec<usize> {
    match operation {
        Operation::Return | Operation::Throw => Vec::new(),
        Operation::Jump(target) => vec![*target],
        Operation::Switch(targets) => targets.clone(),
        Operation::NullBranch {
            jumps_on_null,
            target,
        } if top == Some(Slot::Parameter) => {
            vec![if *jumps_on_null { *target } else { next }]
        }
        Operation::ZeroBranch {
            jumps_on_zero,
            target,
        } if top == Some(Slot::TypeCheck) => {
            vec![if *jumps_on_zero { *target } else { next }]
        }
        Operation::NullBranch { target, .. }
        | Operation::ZeroBranch { target, .. }
        | Operation::Branch { target, .. } => vec![next, *target],
        Operation::Effect { .. }
        | Operation::LoadReference(_)
        | Operation::StoreReference(_)
        | Operation::StorePrimitive { .. }
        | Operation::Duplicate { .. }
        | Operation::Swap
        | Operation::Dereference { .. }
        | Operation::TypeCheck
        | Operation::Invoke(_) => vec![next],
    }
}
