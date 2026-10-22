use super::java::{JavaSummary, Position, lint_at};
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_chains::{Chain, ChainRoot};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const NULLABLE: &str = "Nullable";

// Calca el DataFlowIssue de IntelliJ más común: en una cadena de llamadas sobre clases de
// librería, un paso que la librería declara `@Nullable` seguido de otra llamada sobre su resultado
// (`respuesta.getBody().error()`). El tipo de cada paso sale de la firma del anterior; con
// sobrecargas del mismo nombre y aridad, sólo si todas dicen lo mismo.
pub fn nullable_dereferences(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    if library.is_empty() {
        return Vec::new();
    }
    summary
        .chains
        .iter()
        .flat_map(|chain| dereferences_in(types, library, summary, chain))
        .collect()
}

fn dereferences_in(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Vec<LintFinding> {
    let (ChainRoot::Instance(root) | ChainRoot::Class(root)) = &chain.root;
    let Resolved::Library(mut class) = resolve(types, library, summary, root) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for pair in chain.steps.windows(2) {
        let [step, next] = pair else {
            break;
        };
        let candidates = library.overloads(&class, &step.name, step.arity);
        let Some(first) = candidates.first() else {
            break;
        };
        let nullable = candidates.iter().all(|method| {
            method
                .return_annotations
                .iter()
                .any(|annotation| annotation.rsplit(QUALIFIER_SEPARATOR).next() == Some(NULLABLE))
        });
        if nullable && !step.checked {
            findings.push(lint_at(
                JavaRule::DataFlowIssue,
                &Position {
                    line: next.line,
                    column: next.column,
                },
                format!("{}()", next.name),
            ));
        }
        let returned = first.return_class();
        if candidates.iter().any(|method| method.return_class() != returned) {
            break;
        }
        let Some(returned) = returned else {
            break;
        };
        class = returned;
    }
    findings
}
