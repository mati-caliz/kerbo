use std::path::{Path, PathBuf};

use crate::index::java::JavaSummary;
use crate::index::{IndexedSummary, project_lints};
use crate::jvm::classpath;
use crate::jvm::library::Library;
use crate::limits::Limits;
use crate::lint::LintFinding;
use crate::rules::{Diagnostic, FileIdentity, JavaRule, RuleId, finding_diagnostics};

const LIBRARY_RULES: [JavaRule; 2] = [JavaRule::NullableProblems, JavaRule::DataFlowIssue];

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

// Las firmas de las dependencias sólo se cargan si alguna regla prendida las usa.
pub fn diagnostics(files: &[IndexedFile], root: &Path) -> Vec<Diagnostic> {
    let wants_library = files.iter().any(|file| {
        LIBRARY_RULES
            .iter()
            .any(|rule| file.limits.get(RuleId::Java(*rule)).is_some())
    });
    let jars = if wants_library {
        classpath::for_project(root)
    } else {
        Vec::new()
    };
    let library = Library::load(&jars);
    let summaries: Vec<IndexedSummary<'_>> = files
        .iter()
        .map(|file| IndexedSummary {
            summary: &file.summary,
            in_tests: file.in_tests,
        })
        .collect();
    project_lints(&summaries, &library)
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
