use std::collections::BTreeMap;
use std::path::Path;

use tree_sitter::Point;

use super::{ScanRequest, SourceFile, file_limits};
use crate::language::Language;
use crate::limits::Limits;
use crate::lint::{LintKind, finding_at};
use crate::rules::{Diagnostic, FileIdentity, JavaRule, RuleId, finding_diagnostics};

const PACKAGE_INFO: &str = "package-info.java";
const MODULE_INFO: &str = "module-info.java";
const PATH_SEPARATOR: char = '/';

struct JavaFile<'scan> {
    identity: &'scan FileIdentity,
    limits: Limits,
}

fn file_name(file: &JavaFile<'_>) -> String {
    file.identity
        .absolute
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// Checkstyle avisa en el primer archivo que procesa de la carpeta, y en Windows los recorre en el
// orden de NTFS: alfabético sin distinguir mayúsculas.
fn first_in_folder<'files, 'scan>(files: &'files [JavaFile<'scan>]) -> Option<&'files JavaFile<'scan>> {
    files.iter().min_by_key(|file| {
        let name = file_name(file);
        (name.to_uppercase(), name)
    })
}

fn relative_folder(identity: &FileIdentity) -> &str {
    identity
        .relative
        .rsplit_once(PATH_SEPARATOR)
        .map_or("", |(folder, _)| folder)
}

// JavadocPackage: una carpeta con archivos Java y sin package-info.java en el disco; module-info.java
// no es de ningún paquete.
pub fn diagnostics(files: &[SourceFile], request: &ScanRequest<'_>) -> Vec<Diagnostic> {
    let mut folders: BTreeMap<&Path, Vec<JavaFile<'_>>> = BTreeMap::new();
    let java_files = files
        .iter()
        .filter(|file| file.language == Some(Language::Java))
        .filter(|file| !file.identity.absolute.ends_with(MODULE_INFO));
    for file in java_files {
        if let Some(folder) = file.identity.absolute.parent() {
            folders.entry(folder).or_default().push(JavaFile {
                identity: &file.identity,
                limits: file_limits(file, Language::Java, request),
            });
        }
    }
    folders
        .into_iter()
        .filter(|(folder, _)| !folder.join(PACKAGE_INFO).exists())
        .filter_map(|(_, inside)| {
            let first = first_in_folder(&inside)?;
            first.limits.get(RuleId::Java(JavaRule::JavadocPackage))?;
            let finding = finding_at(
                LintKind::Java(JavaRule::JavadocPackage),
                Point::new(0, 0),
                relative_folder(first.identity),
                None,
            );
            Some(finding_diagnostics(first.identity, &[finding]).collect::<Vec<_>>())
        })
        .flatten()
        .collect()
}
