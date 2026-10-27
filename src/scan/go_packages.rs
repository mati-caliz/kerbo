use std::collections::{HashMap, HashSet};

use crate::analysis::FileAnalysis;
use crate::i18n::Message;
use crate::lint::LintKind;
use crate::lint::go::{GoPackageFacts, message_problems};
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

type Position<'file> = (&'file str, usize, usize);

// Como staticcheck: en los tests sólo se mira lo de afuera de las funciones (las funciones de test
// "son para leer, no parte de una API"), y un mensaje con mayúscula no avisa si su primera palabra
// es el nombre de una función o un método del paquete fuera de los tests. Los mensajes que nombran
// constantes de otro archivo se resuelven acá con las del paquete entero.
pub fn drop_package_names(diagnostics: &mut Vec<Diagnostic>, files: &[GoPackageFile]) {
    let names = package_names(files);
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
    let outside_functions: HashSet<Position<'_>> = files
        .iter()
        .flat_map(|file| {
            file.facts
                .outside_functions
                .iter()
                .map(move |(line, column)| (file.relative.as_str(), *line, *column))
        })
        .collect();
    let resolved = resolved_messages(files, &names);
    diagnostics.retain_mut(|diagnostic| {
        if diagnostic.rule != RuleId::Go(GoRule::St1005) {
            return true;
        }
        let position = (
            diagnostic.relative_path.as_str(),
            diagnostic.line,
            diagnostic.column,
        );
        let in_scope = !is_test(&diagnostic.relative_path) || outside_functions.contains(&position);
        let subject = subject_of(&diagnostic.fingerprint).to_owned();
        let (kept, word) = resolved.get(&position).map_or_else(
            || {
                let names_the_package =
                    exempt.contains(&(position.0, position.1, position.2, subject.as_str()));
                (!names_the_package, None)
            },
            |message| message.verdict(&subject),
        );
        if let Some(word) = word {
            diagnostic.message = Message::Lint {
                kind: LintKind::Go(GoRule::St1005),
                subject: word,
            };
        }
        in_scope && kept
    });
}

fn package_names(files: &[GoPackageFile]) -> HashMap<(&str, &str), HashSet<&str>> {
    let mut names: HashMap<(&str, &str), HashSet<&str>> = HashMap::new();
    for file in files.iter().filter(|file| !is_test(&file.relative)) {
        names
            .entry(package_key(file))
            .or_default()
            .extend(file.facts.declared.iter().map(String::as_str));
    }
    names
}

// Un mensaje que nombraba constantes de otro archivo, ya con el texto del paquete.
struct ResolvedMessage<'file> {
    callee: &'file str,
    punctuated: bool,
    word: Option<String>,
}

impl ResolvedMessage<'_> {
    // Si el aviso queda y, si es el de la mayúscula, la palabra que pasa a nombrar.
    fn verdict(&self, subject: &str) -> (bool, Option<String>) {
        if subject == self.callee {
            return (self.punctuated, None);
        }
        (self.word.is_some(), self.word.clone())
    }
}

fn resolved_messages<'file>(
    files: &'file [GoPackageFile],
    names: &HashMap<(&str, &str), HashSet<&str>>,
) -> HashMap<Position<'file>, ResolvedMessage<'file>> {
    let mut constants: HashMap<(&str, &str), HashMap<&str, &str>> = HashMap::new();
    for file in files {
        constants.entry(package_key(file)).or_default().extend(
            file.facts
                .constants
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );
    }
    let no_constants = HashMap::new();
    files
        .iter()
        .flat_map(|file| {
            let key = package_key(file);
            let package_constants = constants.get(&key).unwrap_or(&no_constants);
            let declared = names.get(&key);
            file.facts.pending.iter().map(move |pending| {
                let (punctuated, word) = pending
                    .resolve(package_constants)
                    .map_or((false, None), |text| message_problems(&text));
                let word =
                    word.filter(|word| !declared.is_some_and(|declared| declared.contains(word.as_str())));
                (
                    (file.relative.as_str(), pending.line, pending.column),
                    ResolvedMessage {
                        callee: pending.callee.as_str(),
                        punctuated,
                        word,
                    },
                )
            })
        })
        .collect()
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
