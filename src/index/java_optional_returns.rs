use crate::lint::java_chains::{Chain, ChainStep};

const ALTERNATIVE_METHOD: &str = "orElse";
const ONLY_ARGUMENT: &[usize] = &[0];

// `opcional.orElse(null)` puede dar null, como lo lee IntelliJ, se haya podido resolver o no el tipo
// de lo que está antes en la cadena: los pasos sin resolver quedan como no nulos.
pub(super) fn with_absent_alternatives(chain: &Chain, mut nullable: Vec<bool>) -> Vec<bool> {
    for (index, step) in chain.steps.iter().enumerate() {
        if !takes_null_alternative(step) {
            continue;
        }
        if nullable.len() <= index {
            nullable.resize(index + 1, false);
        }
        if let Some(found) = nullable.get_mut(index) {
            *found = true;
        }
    }
    nullable
}

fn takes_null_alternative(step: &ChainStep) -> bool {
    step.name == ALTERNATIVE_METHOD
        && step.arity == ONLY_ARGUMENT.len()
        && step.null_arguments == ONLY_ARGUMENT
}
