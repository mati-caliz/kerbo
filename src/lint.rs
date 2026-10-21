mod ecmascript;
mod final_class;
mod final_parameters;
mod go;
mod imports;
mod java;
mod java_style;
mod modifiers;
mod python;
mod recovered;
mod references;

use std::path::Path;

use tree_sitter::{Node, Point};

use crate::analysis::SourceUnit;
use crate::fix::Fix;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::profile::LintStyle;
use crate::rules::{EslintRule, GoRule, JavaRule, PythonRule, RuleId};
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
    Eslint(EslintRule),
    Go(GoRule),
    Java(JavaRule),
}

impl LintKind {
    pub const fn rule(self) -> RuleId {
        match self {
            Self::Eslint(rule) => RuleId::Eslint(rule),
            Self::UnusedImport => RuleId::UnusedImport,
            Self::RedundantImport(_) => RuleId::RedundantImport,
            Self::StarImport => RuleId::StarImport,
            Self::RedundantModifier => RuleId::RedundantModifier,
            Self::FinalParameter => RuleId::FinalParameter,
            Self::FinalClass => RuleId::FinalClass,
            Self::Python(rule) => RuleId::Python(rule),
            Self::Go(rule) => RuleId::Go(rule),
            Self::Java(rule) => RuleId::Java(rule),
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

const DECLARATION_SUFFIXES: [&str; 3] = [".d.ts", ".d.cts", ".d.mts"];
const DECLARATION_MARK: &str = ".d.";
const TYPESCRIPT_SUFFIX: &str = ".ts";

pub const LINT_RULES: [RuleId; 6] = [
    RuleId::UnusedImport,
    RuleId::RedundantImport,
    RuleId::StarImport,
    RuleId::RedundantModifier,
    RuleId::FinalParameter,
    RuleId::FinalClass,
];

pub use go::nolint_ranges as go_nolint_ranges;
pub use java_style::relaxed_by as java_relaxed_by;
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
        LintStyle::GolangciLint => go::findings(root, unit.source, limits),
        LintStyle::Eslint => {
            let file = ecmascript::SourceFile {
                source: unit.source,
                declaration_file: unit.path.is_some_and(is_declaration_file),
            };
            ecmascript::reported(root, file, limits)
        }
    }
}

pub fn eslint_relaxed_by(preset: Preset, language: Language) -> Vec<RuleId> {
    ecmascript::relaxed_by(preset, language)
}

fn is_declaration_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    DECLARATION_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
        || name.find(DECLARATION_MARK).is_some_and(|start| {
            name.get(start..)
                .is_some_and(|rest| rest.len() > DECLARATION_MARK.len() && rest.ends_with(TYPESCRIPT_SUFFIX))
        })
}

fn java_lints(root: Node<'_>, source: &[u8], limits: Limits) -> Vec<LintFinding> {
    let family_off = JavaRule::ALL
        .into_iter()
        .all(|rule| limits.get(RuleId::Java(rule)).is_none());
    if family_off && LINT_RULES.iter().all(|&rule| limits.get(rule).is_none()) {
        return Vec::new();
    }
    let mut findings = imports::reported(root, source);
    findings.extend(modifiers::reported(root, source));
    findings.extend(final_parameters::reported(root, source));
    findings.extend(final_class::reported(root, source));
    findings.extend(java_style::reported(root, source));
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
