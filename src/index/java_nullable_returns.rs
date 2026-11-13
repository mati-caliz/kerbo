use super::java::{JavaSummary, Position, lint_at};
use super::java_dereference::{known_not_null, may_return_null};
use super::java_null_arguments::reaches;
use super::java_nullness::ProjectTypes;
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_null_flow::NullReturn;
use crate::rules::JavaRule;

// Calca el "'null' is returned by the method declared as @NullMarked" de DataFlowIssue y su
// variante "might evaluate to null": un `null` escrito, una variable que puede serlo o una llamada
// que puede devolverlo, devueltos por un método que `@NullMarked` (o un `@NonNull`) da por no nulo.
pub fn nullable_returns(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    let from_flows = summary
        .null_flows
        .iter()
        .filter(|flow| !flow.returns.is_empty())
        .filter(|flow| reaches(types, library, summary, flow))
        .flat_map(|flow| &flow.returns);
    let from_calls = summary.chains.iter().filter_map(|chain| {
        let returned = chain.returned.as_ref()?;
        let last = chain.steps.last()?;
        (may_return_null(types, library, summary, chain) && !known_not_null(types, library, summary, last))
            .then_some(returned)
    });
    from_flows
        .chain(from_calls)
        .filter(|returned| types.is_marked(summary, returned.marking))
        .map(report)
        .collect()
}

fn report(returned: &NullReturn) -> LintFinding {
    lint_at(
        JavaRule::NullableReturn,
        &Position {
            line: returned.line,
            column: returned.column,
        },
        returned.name.clone(),
    )
}
