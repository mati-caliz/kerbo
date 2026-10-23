use std::collections::HashMap;

use super::java::{JavaSummary, lint_at};
use super::java_exposure::{DeclaredType, Exposure, rank};
use super::java_project::IndexedSummary;
use crate::lint::LintFinding;
use crate::lint::java_calls::Visibility;
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';

// Los tipos de nivel superior de cada paquete, por nombre: un tipo de paquete sólo se puede nombrar
// desde su mismo paquete.
pub struct PackageTypes<'summary> {
    by_package: HashMap<&'summary str, HashMap<&'summary str, Vec<Visibility>>>,
}

impl<'summary> PackageTypes<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut by_package: HashMap<&str, HashMap<&str, Vec<Visibility>>> = HashMap::new();
        for file in files {
            let names = by_package.entry(file.summary.package.as_str()).or_default();
            for declared in file
                .summary
                .declared_types
                .iter()
                .filter(|declared| declared.top_level)
            {
                names
                    .entry(declared.name.as_str())
                    .or_default()
                    .push(declared.reach);
            }
        }
        Self { by_package }
    }

    fn top_level(&self, package: &str, name: &str) -> Option<Visibility> {
        match self.by_package.get(package)?.get(name)?.as_slice() {
            [single] => Some(*single),
            _ => None,
        }
    }
}

// `ClassEscapesItsScope` de IntelliJ: un tipo de menor visibilidad en la firma de un miembro que se
// ve desde más lejos. El nombre se resuelve sin adivinar: un parámetro de tipo o un import con el
// mismo nombre lo tapan, y si hay dos tipos que se llaman igual no se marca nada.
pub fn escaped_types(types: &PackageTypes<'_>, summary: &JavaSummary) -> Vec<LintFinding> {
    summary
        .exposures
        .iter()
        .filter_map(|exposure| {
            let name = exposure
                .written
                .rsplit(QUALIFIER_SEPARATOR)
                .next()
                .unwrap_or(&exposure.written);
            let reach = resolve(types, summary, exposure, name)?;
            (rank(exposure.reach) > rank(reach)).then(|| {
                lint_at(
                    JavaRule::ClassEscapesItsScope,
                    &exposure.position,
                    name.to_owned(),
                )
            })
        })
        .collect()
}

fn resolve(
    types: &PackageTypes<'_>,
    summary: &JavaSummary,
    exposure: &Exposure,
    name: &str,
) -> Option<Visibility> {
    if exposure.type_parameters.iter().any(|parameter| parameter == name) {
        return None;
    }
    let nested: Vec<&DeclaredType> = summary
        .declared_types
        .iter()
        .filter(|declared| !declared.top_level && declared.name == name)
        .collect();
    match nested.as_slice() {
        [single] => return Some(single.reach),
        [] => {}
        _ => return None,
    }
    if exposure.written.contains(QUALIFIER_SEPARATOR) {
        return None;
    }
    let imported = summary.imports.iter().any(|path| {
        path.rsplit(QUALIFIER_SEPARATOR)
            .next()
            .is_some_and(|last| last == name)
    });
    if imported {
        return None;
    }
    types.top_level(&summary.package, name)
}
