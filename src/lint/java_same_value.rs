use tree_sitter::Node;

use super::java::text;
use super::java_calls::{Arity, Body, Call, Callable, TYPE_DECLARATIONS, Visibility, extract};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

// Calca SameParameterValue de IntelliJ para lo privado: un método o constructor privado (también
// el de un enum o un record privado) al que todas las llamadas de su clase de nivel superior le
// pasan la misma constante en un parámetro. Si hay otro método con el mismo nombre y la misma
// cantidad de parámetros, o alguien lo usa como referencia (`this::metodo`), no se sabe qué
// llamada va a cuál y no se marca. Lo que no es privado lo mira el índice del proyecto.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    named_children(root)
        .into_iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
        .flat_map(|top_level| {
            let extracted = extract(top_level, source);
            same_values(
                &extracted.callables,
                &extracted.calls,
                &extracted.referenced,
                source,
            )
        })
        .collect()
}

fn same_values(
    callables: &[Callable<'_>],
    calls: &[Call],
    referenced: &[String],
    source: &[u8],
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for target in callables.iter().filter(|callable| is_candidate(callable)) {
        let arity = target.parameters.len();
        let overloaded = callables
            .iter()
            .filter(|other| {
                other.name == target.name
                    && other.is_constructor == target.is_constructor
                    && other.parameters.len() == arity
            })
            .count()
            > 1;
        let is_referenced = !target.is_constructor && referenced.contains(&target.name);
        if overloaded || is_referenced {
            continue;
        }
        let matching: Vec<&Call> = calls
            .iter()
            .filter(|call| {
                call.name == target.name
                    && call.is_constructor == target.is_constructor
                    && call.arguments.len() == arity
            })
            .collect();
        for (index, parameter) in target.parameters.iter().enumerate() {
            if let Some(value) = shared_value(&matching, index) {
                let name = text(*parameter, source);
                findings.push(finding(
                    LintKind::Java(JavaRule::SameParameterValue),
                    *parameter,
                    &format!("{name} = {value}"),
                    None,
                ));
            }
        }
    }
    findings
}

fn is_candidate(callable: &Callable<'_>) -> bool {
    callable.visibility == Visibility::Private
        && callable.body == Body::Written
        && callable.arity == Arity::Fixed
        && !callable.parameters.is_empty()
}

pub fn shared_value(calls: &[&Call], index: usize) -> Option<String> {
    let mut values = calls
        .iter()
        .map(|call| call.arguments.get(index).cloned().flatten());
    let first = values.next()??;
    values
        .all(|value| value.as_deref() == Some(first.as_str()))
        .then_some(first)
}
