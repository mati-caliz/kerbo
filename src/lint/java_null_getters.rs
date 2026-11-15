use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java_chains::Chain;
use super::java_null_flow::Analysis;

const QUALIFIER_SEPARATOR: char = '.';

// Las llamadas sin argumentos (`p.getLastBuild()`) cuyo resultado ya se desreferenció por una
// variable: IntelliJ guarda el valor del getter, y la misma llamada más adelante ya es no nula
// (`b2 = p.getLastBuild()` después de `b1.getNumber()`), hasta que se reasigna la raíz.
#[derive(Default)]
pub(super) struct Getters {
    texts: HashMap<usize, String>,
    present: HashSet<String>,
}

pub(super) fn remember_getter(analysis: &mut Analysis<'_, '_>, flow: usize, chain: &Chain, value: Node<'_>) {
    if chain.steps.iter().all(|step| step.arity == 0) {
        let written = compact(analysis.text(value));
        analysis.getters.texts.insert(flow, written);
    }
}

pub(super) fn is_known_getter(analysis: &Analysis<'_, '_>, value: Node<'_>) -> bool {
    analysis.getters.present.contains(&compact(analysis.text(value)))
}

pub(super) fn note_dereferenced(analysis: &mut Analysis<'_, '_>, flow: usize) {
    if let Some(written) = analysis.getters.texts.get(&flow).cloned() {
        analysis.getters.present.insert(written);
    }
}

pub(super) fn forget_getters_of(analysis: &mut Analysis<'_, '_>, variable: &str) {
    let prefix = format!("{variable}{QUALIFIER_SEPARATOR}");
    analysis
        .getters
        .present
        .retain(|written| !written.starts_with(&prefix));
}

fn compact(written: &str) -> String {
    written.split_whitespace().collect()
}
