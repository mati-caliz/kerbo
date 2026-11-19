use super::java::JavaSummary;
use crate::lint::LintFinding;
use crate::rules::{JavaRule, RuleId};

// Los nombres con que `@SuppressWarnings` apaga DataFlowIssue en IntelliJ: el actual y el viejo.
const DATA_FLOW_NAMES: &[&str] = &["DataFlowIssue", "ConstantConditions"];
const DATA_FLOW_RULES: &[JavaRule] = &[
    JavaRule::DataFlowIssue,
    JavaRule::NullArgument,
    JavaRule::AlreadyAssigned,
    JavaRule::FailingCall,
    JavaRule::NullableReturn,
    JavaRule::RedundantNullable,
];

// Saca lo que cae dentro de una declaración con `@SuppressWarnings("ConstantConditions")` (o
// `"DataFlowIssue"`): Qodana no lo informa.
pub(super) fn drop_inspection_suppressed(findings: &mut Vec<LintFinding>, summary: &JavaSummary) {
    if summary.suppressed_spans.is_empty() {
        return;
    }
    findings.retain(|finding| {
        let is_data_flow = DATA_FLOW_RULES
            .iter()
            .any(|rule| finding.kind.rule() == RuleId::Java(*rule));
        let position = (finding.line, finding.column);
        !(is_data_flow
            && summary.suppressed_spans.iter().any(|span| {
                span.start <= position
                    && position < span.end
                    && span
                        .names
                        .iter()
                        .any(|name| DATA_FLOW_NAMES.contains(&name.as_str()))
            }))
    });
}
