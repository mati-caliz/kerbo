use std::collections::HashMap;

use super::{Diagnostic, FileIdentity, RuleId};
use crate::analysis::FileAnalysis;
use crate::i18n::Message;
use crate::lint::LintFinding;

const FOUND: usize = 1;
const ALLOWED: usize = 0;

pub fn lint_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    finding_diagnostics(file, &analysis.lints)
}

pub fn finding_diagnostics<'found>(
    file: &'found FileIdentity,
    findings: &'found [LintFinding],
) -> impl Iterator<Item = Diagnostic> + 'found {
    let mut occurrences: HashMap<(RuleId, &str), usize> = HashMap::new();
    findings.iter().map(move |found| {
        let rule = found.kind.rule();
        let occurrence = occurrences.entry((rule, found.subject.as_str())).or_default();
        *occurrence = occurrence.saturating_add(1);
        Diagnostic {
            path: file.display.clone(),
            line: found.line,
            column: found.column,
            end_line: found.line,
            rule,
            message: message(found),
            value: FOUND,
            limit: ALLOWED,
            fingerprint: format!(
                "{}:{}:{}#{occurrence}",
                rule.as_str(),
                file.relative,
                found.subject
            ),
            relative_path: file.relative.clone(),
            absolute_path: file.absolute.clone(),
            fix: found.fix.clone(),
        }
    })
}

fn message(found: &LintFinding) -> Message {
    Message::Lint {
        kind: found.kind,
        subject: found.subject.clone(),
    }
}
