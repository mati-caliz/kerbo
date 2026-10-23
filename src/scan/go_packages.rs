use std::collections::{HashMap, HashSet};

use crate::analysis::FileAnalysis;
use crate::lint::go::GoPackageFacts;
use crate::rules::{Diagnostic, FileIdentity, GoRule, RuleId};

const TEST_SUFFIX: &str = "_test.go";
const PATH_SEPARATOR: char = '/';
const FINGERPRINT_SEPARATOR: char = ':';
const OCCURRENCE_SEPARATOR: char = '#';

// Un archivo Go con lo que aporta a ST1005 en su paquete.
#[derive(Debug)]
pub struct GoPackageFile {
    relative: String,
    facts: GoPackageFacts,
}

pub fn package_file(identity: &FileIdentity, analysis: &mut FileAnalysis) -> Option<GoPackageFile> {
    analysis.go_package.take().map(|facts| GoPackageFile {
        relative: identity.relative.clone(),
        facts,
    })
}

// Un paquete de Go: la carpeta y el nombre de `package`.
fn package_key(file: &GoPackageFile) -> (&str, &str) {
    let directory = file
        .relative
        .rsplit_once(PATH_SEPARATOR)
        .map_or("", |(directory, _)| directory);
    (directory, file.facts.package.as_str())
}

fn is_test(relative: &str) -> bool {
    relative.ends_with(TEST_SUFFIX)
}

// Como staticcheck: en los tests sólo se mira lo de afuera de las funciones (las funciones de test
// "son para leer, no parte de una API"), y un mensaje con
// mayúscula no avisa si su primera palabra es el nombre de una función o un método del paquete
// fuera de los tests.
pub fn drop_package_names(diagnostics: &mut Vec<Diagnostic>, files: &[GoPackageFile]) {
    let mut names: HashMap<(&str, &str), HashSet<&str>> = HashMap::new();
    for file in files.iter().filter(|file| !is_test(&file.relative)) {
        names
            .entry(package_key(file))
            .or_default()
            .extend(file.facts.declared.iter().map(String::as_str));
    }
    let exempt: HashSet<(&str, usize, usize, &str)> = files
        .iter()
        .flat_map(|file| {
            let declared = names.get(&package_key(file));
            file.facts
                .capitalized
                .iter()
                .filter(move |found| declared.is_some_and(|declared| declared.contains(found.word.as_str())))
                .map(move |found| {
                    (
                        file.relative.as_str(),
                        found.line,
                        found.column,
                        found.word.as_str(),
                    )
                })
        })
        .collect();
    let outside_functions: HashSet<(&str, usize, usize)> = files
        .iter()
        .flat_map(|file| {
            file.facts
                .outside_functions
                .iter()
                .map(move |(line, column)| (file.relative.as_str(), *line, *column))
        })
        .collect();
    diagnostics.retain(|diagnostic| {
        let position = (
            diagnostic.relative_path.as_str(),
            diagnostic.line,
            diagnostic.column,
        );
        diagnostic.rule != RuleId::Go(GoRule::St1005)
            || ((!is_test(&diagnostic.relative_path) || outside_functions.contains(&position))
                && !exempt.contains(&(
                    position.0,
                    position.1,
                    position.2,
                    subject_of(&diagnostic.fingerprint),
                )))
    });
}

// La huella es `regla:ruta:sujeto#n`; el sujeto de un mensaje con mayúscula es su primera palabra.
fn subject_of(fingerprint: &str) -> &str {
    fingerprint
        .rsplit_once(OCCURRENCE_SEPARATOR)
        .map_or(fingerprint, |(before, _)| before)
        .rsplit_once(FINGERPRINT_SEPARATOR)
        .map_or("", |(_, subject)| subject)
}

#[cfg(test)]
mod tests;
