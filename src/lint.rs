mod final_class;
mod final_parameters;
mod imports;
mod java;
mod modifiers;
mod python;
mod recovered;
mod references;

use tree_sitter::{Node, Point};

use crate::analysis::SourceUnit;
use crate::fix::Fix;
use crate::limits::Limits;
use crate::profile::LintStyle;
use crate::rules::{PythonRule, RuleId};
use crate::syntax::one_based;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRedundancy {
    JavaLang,
    SamePackage,
    Duplicate { first_line: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintKind {
    UnusedImport,
    RedundantImport(ImportRedundancy),
    StarImport,
    RedundantModifier,
    FinalParameter,
    FinalClass,
    Python(PythonRule),
}

impl LintKind {
    pub const fn rule(self) -> RuleId {
        match self {
            Self::UnusedImport => RuleId::UnusedImport,
            Self::RedundantImport(_) => RuleId::RedundantImport,
            Self::StarImport => RuleId::StarImport,
            Self::RedundantModifier => RuleId::RedundantModifier,
            Self::FinalParameter => RuleId::FinalParameter,
            Self::FinalClass => RuleId::FinalClass,
            Self::Python(rule) => RuleId::Python(rule),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFinding {
    pub kind: LintKind,
    pub line: usize,
    pub column: usize,
    pub subject: String,
    pub fix: Option<Fix>,
}

pub const LINT_RULES: [RuleId; 6] = [
    RuleId::UnusedImport,
    RuleId::RedundantImport,
    RuleId::StarImport,
    RuleId::RedundantModifier,
    RuleId::FinalParameter,
    RuleId::FinalClass,
];

pub use python::relaxed_by as python_relaxed_by;

pub fn find_lints(
    root: Node<'_>,
    unit: SourceUnit<'_>,
    style: LintStyle,
    limits: Limits,
) -> Vec<LintFinding> {
    match style {
        LintStyle::Unmeasured => Vec::new(),
        LintStyle::Checkstyle => java_lints(root, unit.source, limits),
        LintStyle::Ruff => python::findings(root, unit.source, limits, unit.path),
    }
}

fn java_lints(root: Node<'_>, source: &[u8], limits: Limits) -> Vec<LintFinding> {
    if LINT_RULES.iter().all(|&rule| limits.get(rule).is_none()) {
        return Vec::new();
    }
    let mut findings = imports::reported(root, source);
    findings.extend(modifiers::reported(root, source));
    findings.extend(final_parameters::reported(root, source));
    findings.extend(final_class::reported(root, source));
    findings.retain(|finding| limits.get(finding.kind.rule()).is_some());
    findings.sort_by_key(|finding| (finding.line, finding.column));
    findings
}

fn finding(kind: LintKind, position: Node<'_>, subject: &str, fix: Option<Fix>) -> LintFinding {
    finding_at(kind, position.start_position(), subject, fix)
}

fn finding_at(kind: LintKind, start: Point, subject: &str, fix: Option<Fix>) -> LintFinding {
    LintFinding {
        kind,
        line: one_based(start.row),
        column: one_based(start.column),
        subject: subject.to_owned(),
        fix,
    }
}

#[cfg(test)]
mod tests;
