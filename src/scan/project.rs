use std::path::{Path, PathBuf};

use crate::index::java::JavaSummary;
use crate::index::{IndexedSummary, project_lints};
use crate::limits::Limits;
use crate::lint::LintFinding;
use crate::rules::{Diagnostic, FileIdentity, finding_diagnostics};

#[derive(Debug)]
pub struct IndexedFile {
    pub identity: FileIdentity,
    pub limits: Limits,
    pub summary: JavaSummary,
    pub in_tests: bool,
}

// Las reglas que miran el proyecto entero sólo corren si el análisis lo abarca: con una carpeta o
// un archivo suelto faltarían llamadas y avisarían de más.
pub fn covers_project(roots: &[PathBuf], project_root: &Path) -> bool {
    roots
        .iter()
        .any(|root| root.canonicalize().is_ok_and(|root| root == project_root))
}

pub fn diagnostics(files: &[IndexedFile]) -> Vec<Diagnostic> {
    let summaries: Vec<IndexedSummary<'_>> = files
        .iter()
        .map(|file| IndexedSummary {
            summary: &file.summary,
            in_tests: file.in_tests,
        })
        .collect();
    project_lints(&summaries)
        .into_iter()
        .zip(files)
        .flat_map(|(findings, file)| {
            let kept: Vec<LintFinding> = findings
                .into_iter()
                .filter(|finding| file.limits.get(finding.kind.rule()).is_some())
                .collect();
            finding_diagnostics(&file.identity, &kept).collect::<Vec<_>>()
        })
        .collect()
}
