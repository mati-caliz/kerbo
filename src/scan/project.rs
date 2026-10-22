use std::path::{Path, PathBuf};

use crate::analysis::FileAnalysis;
use crate::index::java::JavaSummary;
use crate::index::python_taint::{PythonModule, python_tainted};
use crate::index::{IndexedSummary, project_lints};
use crate::jvm::classpath;
use crate::jvm::library::Library;
use crate::limits::Limits;
use crate::lint::LintFinding;
use crate::lint::taint::TaintMethod;
use crate::rules::{Diagnostic, FileIdentity, JavaRule, RuleId, finding_diagnostics};

const LIBRARY_RULES: [JavaRule; 3] = [
    JavaRule::NullableProblems,
    JavaRule::DataFlowIssue,
    JavaRule::AutoCloseableResource,
];

// Un archivo de Python con lo que hacen sus funciones con los datos.
#[derive(Debug)]
pub struct PythonFile {
    pub identity: FileIdentity,
    pub limits: Limits,
    pub taints: Vec<TaintMethod>,
}

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

// Lo que llega del usuario a las consultas, comandos, rutas y respuestas de los archivos de Python.
pub fn python_diagnostics(files: &[PythonFile]) -> Vec<Diagnostic> {
    let modules: Vec<PythonModule<'_>> = files
        .iter()
        .map(|file| PythonModule {
            path: &file.identity.relative,
            taints: &file.taints,
        })
        .collect();
    python_tainted(&modules)
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

// Lo que el archivo aporta a las reglas que miran el proyecto entero.
pub fn summaries(
    identity: &FileIdentity,
    in_tests: bool,
    analysis: &mut FileAnalysis,
    limits: Limits,
) -> (Option<IndexedFile>, Option<PythonFile>) {
    let indexed = analysis.java_summary.take().map(|summary| IndexedFile {
        identity: identity.clone(),
        limits,
        summary,
        in_tests,
    });
    let python = analysis.python_taints.take().map(|taints| PythonFile {
        identity: identity.clone(),
        limits,
        taints,
    });
    (indexed, python)
}
