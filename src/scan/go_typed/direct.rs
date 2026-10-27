use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::go_types::{ModuleFacts, ReportFact};
use crate::language::Language;
use crate::limits::Limits;
use crate::lint::go::is_generated_source;
use crate::lint::{LintFinding, LintKind};
use crate::rules::{Diagnostic, FileIdentity, GoRule, RuleId, finding_diagnostics};
use crate::scan::{ScanRequest, SourceFile, file_limits};
use crate::suppressions::{Suppressed, split};

// Las reglas que el helper decide entero: aparecen poco y cada una pide tipos para saber si va, así
// que no hay candidatos por archivo; el aviso sale directo de los hechos.
pub const DIRECT_RULES: [GoRule; 10] = [
    GoRule::Sa1019,
    GoRule::Qf1001,
    GoRule::Stdmethods,
    GoRule::St1012,
    GoRule::Qf1011,
    GoRule::St1023,
    GoRule::Qf1002,
    GoRule::S1005,
    GoRule::Sa9003,
    GoRule::Qf1006,
];

// Un archivo Go del escaneo con alguna regla directa prendida.
#[derive(Debug)]
pub struct DirectFile<'scan> {
    pub identity: &'scan FileIdentity,
    pub limits: Limits,
}

// Los archivos Go del escaneo con alguna regla directa prendida.
pub fn direct_files<'scan>(files: &'scan [SourceFile], request: &ScanRequest<'_>) -> Vec<DirectFile<'scan>> {
    files
        .iter()
        .filter(|file| file.language == Some(Language::Go))
        .map(|file| DirectFile {
            identity: &file.identity,
            limits: file_limits(file, Language::Go, request),
        })
        .filter(|file| wants_direct_reports(&file.limits))
        .collect()
}

pub fn wants_direct_reports(limits: &Limits) -> bool {
    DIRECT_RULES
        .iter()
        .any(|rule| limits.get(RuleId::Go(*rule)).is_some())
}

// Los avisos de las reglas directas de cada archivo, con sus límites, sin los archivos generados (como
// golangci-lint) y pasados por los `//nolint` del archivo.
pub fn direct_reports(
    files: &[DirectFile<'_>],
    roots: &HashMap<PathBuf, PathBuf>,
    modules: &HashMap<&Path, ModuleFacts>,
) -> (Vec<Diagnostic>, Vec<Suppressed>) {
    let mut kept = Vec::new();
    let mut suppressed = Vec::new();
    for file in files {
        let Some(facts) = roots
            .get(&file.identity.absolute)
            .and_then(|root| modules.get(root.as_path()))
            .and_then(|module| module.file(&file.identity.absolute))
        else {
            continue;
        };
        let findings: Vec<LintFinding> = facts
            .reports
            .iter()
            .flatten()
            .filter_map(|report| finding(report, &file.limits))
            .collect();
        if findings.is_empty() {
            continue;
        }
        let Ok(source) = fs::read(&file.identity.absolute) else {
            continue;
        };
        if is_generated_source(&source) {
            continue;
        }
        let (file_kept, file_suppressed) =
            split(&source, finding_diagnostics(file.identity, &findings).collect());
        kept.extend(file_kept);
        suppressed.extend(file_suppressed);
    }
    (kept, suppressed)
}

fn finding(report: &ReportFact, limits: &Limits) -> Option<LintFinding> {
    let rule = DIRECT_RULES
        .into_iter()
        .find(|rule| rule.as_str() == report.rule)?;
    limits.get(RuleId::Go(rule))?;
    Some(LintFinding {
        kind: LintKind::Go(rule),
        line: report.line,
        column: report.column,
        subject: report.subject.clone(),
        fix: None,
    })
}

#[cfg(test)]
mod tests;
