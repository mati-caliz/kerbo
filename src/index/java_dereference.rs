use super::java::{JavaSummary, Position, lint_at};
use super::java_jdk::jdk_method;
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_chains::{Chain, ChainRoot};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const NULLABLE: &str = "Nullable";

// Calca el DataFlowIssue de IntelliJ en sus dos formas sin flujo entre métodos: en una cadena de
// llamadas, un paso que la librería declara `@Nullable` seguido de otra llamada sobre su resultado
// (`respuesta.getBody().error()`), y una variable asignada desde una llamada `@Nullable` usada sin
// chequeo en algún camino (`Cache cache = manager.getCache(nombre); cache.clear();`).
pub fn nullable_dereferences(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = summary
        .chains
        .iter()
        .flat_map(|chain| {
            let nullable = nullable_steps(types, library, summary, chain);
            let unboxing = chain
                .unboxed
                .zip(chain.steps.last())
                .filter(|(_, last)| {
                    !last.checked && nullable.len() == chain.steps.len() && nullable.last() == Some(&true)
                })
                .map(|((line, column), last)| report(line, column, &last.name));
            chain
                .steps
                .windows(2)
                .zip(nullable.iter().copied())
                .filter_map(|(pair, is_nullable)| match pair {
                    [step, next] if is_nullable && !step.checked => {
                        Some(report(next.line, next.column, &next.name))
                    }
                    _ => None,
                })
                .chain(unboxing)
                .collect::<Vec<_>>()
        })
        .collect();
    for flow in &summary.null_flows {
        let nullable = nullable_steps(types, library, summary, &flow.origin);
        if nullable.len() == flow.origin.steps.len() && nullable.last() == Some(&true) {
            findings.extend(
                flow.dereferences
                    .iter()
                    .map(|used| report(used.line, used.column, &used.name)),
            );
        }
    }
    findings
}

fn report(line: usize, column: usize, name: &str) -> LintFinding {
    lint_at(
        JavaRule::DataFlowIssue,
        &Position { line, column },
        format!("{name}()"),
    )
}

// Por cada paso de la cadena que se pudo resolver en la librería, si devuelve `@Nullable`. El tipo
// de cada paso sale de la firma del anterior; con sobrecargas del mismo nombre y aridad, sólo si
// todas dicen lo mismo.
fn nullable_steps(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Vec<bool> {
    let (ChainRoot::Instance(root) | ChainRoot::Class(root)) = &chain.root;
    let mut classes = match resolve(types, library, summary, root) {
        Resolved::Library(class) => vec![class],
        Resolved::Project(name) => library_ancestors(types, library, &name),
    };
    let mut nullable = Vec::new();
    for step in &chain.steps {
        let overloads = classes
            .iter()
            .map(|class| library.overloads(class, &step.name, step.arity))
            .find(|found| !found.is_empty())
            .unwrap_or_default();
        let by_class_arguments: Vec<_> = overloads
            .iter()
            .filter(|method| {
                step.class_arguments
                    .iter()
                    .all(|position| method.takes_class_at(*position))
            })
            .cloned()
            .collect();
        let candidates = if by_class_arguments.is_empty() {
            overloads
        } else {
            by_class_arguments
        };
        let Some(first) = candidates.first() else {
            let Some((returned, is_nullable)) = jdk_method(&classes, &step.name, step.arity) else {
                break;
            };
            nullable.push(is_nullable);
            classes = vec![returned.to_owned()];
            continue;
        };
        nullable.push(candidates.iter().all(|method| {
            method
                .return_annotations
                .iter()
                .any(|annotation| annotation.rsplit(QUALIFIER_SEPARATOR).next() == Some(NULLABLE))
        }));
        let returned = first.return_class();
        if candidates.iter().any(|method| method.return_class() != returned) {
            break;
        }
        let Some(returned) = returned else {
            break;
        };
        classes = vec![returned];
    }
    nullable
}
