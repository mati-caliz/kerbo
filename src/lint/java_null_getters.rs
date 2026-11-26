use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::java::NAME_FIELD;
use super::java_chains::Chain;
use super::java_null_checks::is_getter_name;
use super::java_null_conditions::without_cast;
use super::java_null_flow::Analysis;

const QUALIFIER_SEPARATOR: char = '.';
const INVOCATION: &str = "method_invocation";
const ARGUMENTS_FIELD: &str = "arguments";

// Las llamadas sin argumentos (`p.getLastBuild()`) cuyo resultado ya se desreferenció por una
// variable: IntelliJ guarda el valor del getter, y la misma llamada más adelante ya es no nula
// (`b2 = p.getLastBuild()` después de `b1.getNumber()`), hasta que se reasigna la raíz.
#[derive(Default)]
pub(super) struct Getters {
    texts: HashMap<usize, String>,
    present: HashSet<String>,
    checked: HashSet<String>,
    in_condition: bool,
}

// Asignado a la misma variable de la que sale (`fp = fp.getParent()`), el getter ya es de otro
// objeto: no se recuerda. El flujo del getter es el que se crea a continuación.
pub(super) fn remember_getter(
    analysis: &mut Analysis<'_, '_>,
    chain: &Chain,
    value: Node<'_>,
    assigned: &[String],
) {
    let is_getter = chain.steps.last().is_some_and(|step| is_getter_name(&step.name));
    let written = compact(analysis.text(value));
    let reads_what_it_assigns = assigned
        .iter()
        .any(|variable| written.starts_with(&format!("{variable}{QUALIFIER_SEPARATOR}")));
    if is_getter && !reads_what_it_assigns && chain.steps.iter().all(|step| step.arity == 0) {
        analysis.getters.texts.insert(analysis.flows.len(), written);
    }
}

pub(super) fn is_known_getter(analysis: &Analysis<'_, '_>, value: Node<'_>) -> bool {
    let written = compact(analysis.text(value));
    analysis.getters.present.contains(&written) || analysis.getters.checked.contains(&written)
}

// Un getter sin argumentos comparado con null en la condición de un `if` (`r.getResult() != null`):
// IntelliJ recuerda su valor, y la misma llamada más adelante en la rama vale lo mismo. Se anota en
// las dos ramas de la condición: sólo calla de más.
pub(super) fn note_checked(analysis: &mut Analysis<'_, '_>, compared: Node<'_>) {
    let call = without_cast(compared);
    let is_plain_getter = call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| is_getter_name(analysis.text(name)))
        && call
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| arguments.named_child_count() == 0);
    if is_plain_getter && analysis.getters.in_condition {
        let written = compact(analysis.text(call));
        analysis.getters.checked.insert(written);
    }
}

pub(super) fn begin_condition(analysis: &mut Analysis<'_, '_>) -> HashSet<String> {
    analysis.getters.in_condition = true;
    analysis.getters.checked.clone()
}

pub(super) fn end_condition(analysis: &mut Analysis<'_, '_>, checked_before: HashSet<String>) {
    analysis.getters.in_condition = false;
    analysis.getters.checked = checked_before;
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
