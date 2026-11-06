use crate::go_types::FileFacts;
use crate::limits::Limits;
use crate::lint::go::platform::GoPlatform;
use crate::lint::{LintFinding, LintKind};
use crate::rules::{GoRule, RuleId};

use super::{code, excluded};

// `check-blank` y `check-type-assertions` de errcheck: las dos vienen apagadas y el helper ya sabe
// dónde está cada caso, así que no hay candidatos por archivo.
pub fn findings(facts: &FileFacts, limits: &Limits, platform: &GoPlatform) -> Vec<LintFinding> {
    if limits.get(RuleId::Go(GoRule::Errcheck)).is_none() {
        return Vec::new();
    }
    let mut found = Vec::new();
    if platform.errcheck_check_blank() {
        found.extend(blank_findings(facts, platform));
    }
    if platform.errcheck_check_type_assertions() {
        found.extend(assertion_findings(facts, platform));
    }
    found
}

fn blank_findings(facts: &FileFacts, platform: &GoPlatform) -> Vec<LintFinding> {
    let extra_excluded = platform.errcheck_exclude_functions();
    facts
        .blanks
        .iter()
        .flatten()
        .filter(|blank| {
            !blank
                .call
                .as_ref()
                .is_some_and(|call| excluded(call, extra_excluded))
        })
        .map(|blank| {
            let subject = blank.call.as_ref().map(code).unwrap_or_default();
            errcheck_finding((blank.line, blank.column), subject)
        })
        .collect()
}

fn assertion_findings(facts: &FileFacts, platform: &GoPlatform) -> Vec<LintFinding> {
    facts
        .assertions
        .iter()
        .flatten()
        .filter(|assertion| !assertion.blank || platform.errcheck_check_blank())
        .map(|assertion| errcheck_finding((assertion.line, assertion.column), String::new()))
        .collect()
}

const fn errcheck_finding((line, column): (usize, usize), subject: String) -> LintFinding {
    LintFinding {
        kind: LintKind::Go(GoRule::Errcheck),
        line,
        column,
        subject,
        fix: None,
    }
}

#[cfg(test)]
mod tests;
