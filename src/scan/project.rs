use std::path::{Path, PathBuf};

use crate::analysis::FileAnalysis;
use crate::index::ecmascript_taint::{ScriptModule, ecmascript_tainted};
use crate::index::go_taint::{GoFile, go_tainted};
use crate::index::java::JavaSummary;
use crate::index::python_taint::{PythonModule, python_tainted};
use crate::index::templates::Templates;
use crate::index::{IndexedSummary, project_lints};
use crate::jvm::classpath;
use crate::jvm::library::Library;
use crate::language::Language;
use crate::limits::Limits;
use crate::lint::LintFinding;
use crate::lint::taint::{TaintFamily, TaintMethod};
use crate::rules::{Diagnostic, FileIdentity, JavaRule, RuleId, finding_diagnostics};

const LIBRARY_RULES: [JavaRule; 3] = [
    JavaRule::NullableProblems,
    JavaRule::DataFlowIssue,
    JavaRule::AutoCloseableResource,
];

// Un archivo de Python, JavaScript o TypeScript con lo que hacen sus funciones con los datos.
#[derive(Debug)]
pub struct TaintedFile {
    pub identity: FileIdentity,
    pub limits: Limits,
    pub family: TaintFamily,
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

// Lo que llega del usuario a las consultas, comandos, rutas y respuestas, lenguaje por lenguaje.
pub fn tainted_diagnostics(files: &[TaintedFile], root: &Path) -> Vec<Diagnostic> {
    let of_family = |family: TaintFamily| -> Vec<&TaintedFile> {
        files.iter().filter(|file| file.family == family).collect()
    };
    let templates = Templates::load(root, files.iter().flat_map(|file| file.taints.iter()));
    let mut found = python_diagnostics(&of_family(TaintFamily::Python), &templates);
    found.extend(ecmascript_diagnostics(
        &of_family(TaintFamily::Ecmascript),
        &templates,
    ));
    found.extend(go_diagnostics(&of_family(TaintFamily::Go)));
    found
}

fn go_diagnostics(files: &[&TaintedFile]) -> Vec<Diagnostic> {
    let packages: Vec<GoFile<'_>> = files
        .iter()
        .map(|file| GoFile {
            path: &file.identity.relative,
            taints: &file.taints,
        })
        .collect();
    kept_diagnostics(go_tainted(&packages), files)
}

fn python_diagnostics(files: &[&TaintedFile], templates: &Templates) -> Vec<Diagnostic> {
    let modules: Vec<PythonModule<'_>> = files
        .iter()
        .map(|file| PythonModule {
            path: &file.identity.relative,
            taints: &file.taints,
        })
        .collect();
    kept_diagnostics(python_tainted(&modules, templates), files)
}

// Lo que el archivo aporta a las reglas que miran el proyecto entero.
pub fn summaries(
    identity: &FileIdentity,
    in_tests: bool,
    analysis: &mut FileAnalysis,
    limits: Limits,
) -> (Option<IndexedFile>, Option<TaintedFile>) {
    let indexed = analysis.java_summary.take().map(|summary| IndexedFile {
        identity: identity.clone(),
        limits,
        summary,
        in_tests,
    });
    let tainted = Language::from_path(Path::new(&identity.relative))
        .and_then(TaintFamily::of)
        .zip(analysis.taints.take())
        .map(|(family, taints)| TaintedFile {
            identity: identity.clone(),
            limits,
            family,
            taints,
        });
    (indexed, tainted)
}

fn ecmascript_diagnostics(files: &[&TaintedFile], templates: &Templates) -> Vec<Diagnostic> {
    let modules: Vec<ScriptModule<'_>> = files
        .iter()
        .map(|file| ScriptModule {
            path: &file.identity.relative,
            taints: &file.taints,
        })
        .collect();
    kept_diagnostics(ecmascript_tainted(&modules, templates), files)
}

fn kept_diagnostics(found: Vec<Vec<LintFinding>>, files: &[&TaintedFile]) -> Vec<Diagnostic> {
    found
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
