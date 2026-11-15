use std::collections::HashMap;

use super::java::{JavaSummary, lint_at};
use super::java_annotation_values::{AnnotationValue, qualified};
use super::java_project::IndexedSummary;
use crate::lint::LintFinding;
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const WILDCARD_SUFFIX: &str = ".*";

// Los valores por defecto de las anotaciones que declara el proyecto, por nombre calificado y
// elemento.
pub struct AnnotationDefaults<'summary> {
    by_annotation: HashMap<&'summary str, HashMap<&'summary str, &'summary str>>,
}

impl<'summary> AnnotationDefaults<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut by_annotation: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
        for declared in files.iter().flat_map(|file| &file.summary.annotation_defaults) {
            by_annotation
                .entry(declared.annotation.as_str())
                .or_default()
                .insert(declared.element.as_str(), declared.value.as_str());
        }
        Self { by_annotation }
    }

    fn resolve(
        &self,
        summary: &JavaSummary,
        value: &AnnotationValue,
    ) -> Option<&HashMap<&'summary str, &'summary str>> {
        let mut found = candidates(summary, value)
            .into_iter()
            .filter_map(|candidate| self.by_annotation.get_key_value(candidate.as_str()));
        let (first_name, first) = found.next()?;
        found.all(|(name, _)| name == first_name).then_some(first)
    }
}

// `DefaultAnnotationParam` de IntelliJ: un elemento de una anotación al que se le escribe el mismo
// valor que ya tiene por defecto. Sólo las anotaciones del proyecto, que es donde se ve el default.
pub fn redundant_annotation_params(
    defaults: &AnnotationDefaults<'_>,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    summary
        .annotation_values
        .iter()
        .filter(|value| {
            defaults
                .resolve(summary, value)
                .and_then(|elements| elements.get(value.element.as_str()))
                .is_some_and(|default| *default == value.value)
        })
        .map(|value| {
            lint_at(
                JavaRule::DefaultAnnotationParam,
                &value.position,
                value.element.clone(),
            )
        })
        .collect()
}

// Los nombres calificados a los que puede referirse lo escrito: por un import del primer tramo, por
// un import con comodín, por un tipo de las clases que encierran el uso o por el mismo paquete.
fn candidates(summary: &JavaSummary, value: &AnnotationValue) -> Vec<String> {
    let written = value.written.as_str();
    let (first, rest) = written
        .split_once(QUALIFIER_SEPARATOR)
        .map_or((written, None), |(first, rest)| (first, Some(rest)));
    let with_rest = |prefix: &str| {
        rest.map_or_else(
            || prefix.to_owned(),
            |rest| format!("{prefix}{QUALIFIER_SEPARATOR}{rest}"),
        )
    };
    let mut found: Vec<String> = summary
        .imports
        .iter()
        .filter_map(|import| {
            import.strip_suffix(WILDCARD_SUFFIX).map_or_else(
                || {
                    import
                        .rsplit(QUALIFIER_SEPARATOR)
                        .next()
                        .filter(|last| *last == first)
                        .map(|_| with_rest(import))
                },
                |package| Some(format!("{package}{QUALIFIER_SEPARATOR}{written}")),
            )
        })
        .collect();
    let mut path: Vec<String> = Vec::new();
    for enclosing in std::iter::once(None).chain(value.enclosing.iter().map(Some)) {
        path.extend(enclosing.cloned());
        let mut candidate = path.clone();
        candidate.push(written.to_owned());
        found.push(qualified(&summary.package, &candidate));
    }
    found.push(written.to_owned());
    found
}
