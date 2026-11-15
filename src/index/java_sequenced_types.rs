use super::java::{JavaSummary, Position, lint_at};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_sequenced_collection::{SequencedCandidate, is_sequenced_name};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const WILDCARD_SUFFIX: &str = ".*";
const JAVA_LANG: &str = "java.lang";

// `SequencedCollectionMethodCanBeUsed` cuando el receptor se declara con un tipo que no está en las
// tablas de la regla por archivo: se busca la clase en las dependencias, por los imports y el
// paquete, y vale si alguno de sus padres es una lista (o una colección secuenciada).
pub fn sequenced_on_library_types(library: &Library, summary: &JavaSummary) -> Vec<LintFinding> {
    summary
        .sequenced_candidates
        .iter()
        .filter(|candidate| is_sequenced_in_library(library, summary, candidate))
        .map(|candidate| {
            lint_at(
                JavaRule::SequencedCollectionMethod,
                &Position {
                    line: candidate.line,
                    column: candidate.column,
                },
                candidate.method.clone(),
            )
        })
        .collect()
}

fn is_sequenced_in_library(library: &Library, summary: &JavaSummary, candidate: &SequencedCandidate) -> bool {
    qualified_candidates(summary, &candidate.type_name)
        .into_iter()
        .find(|qualified| library.class_or_jdk(qualified).is_some())
        .is_some_and(|qualified| {
            library.supertypes(&qualified).iter().any(|supertype| {
                let simple = supertype.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(supertype);
                is_sequenced_name(simple, candidate.needs_list)
            })
        })
}

fn qualified_candidates(summary: &JavaSummary, written: &str) -> Vec<String> {
    let (first, rest) = written
        .split_once(QUALIFIER_SEPARATOR)
        .map_or((written, None), |(first, rest)| (first, Some(rest)));
    let with_rest = |prefix: String| {
        rest.map_or_else(
            || prefix.clone(),
            |rest| format!("{prefix}{QUALIFIER_SEPARATOR}{rest}"),
        )
    };
    let mut found: Vec<String> = summary
        .imports
        .iter()
        .filter(|import| import.rsplit(QUALIFIER_SEPARATOR).next() == Some(first))
        .map(|import| with_rest(import.clone()))
        .collect();
    found.extend(
        summary
            .imports
            .iter()
            .filter_map(|import| import.strip_suffix(WILDCARD_SUFFIX))
            .chain([summary.package.as_str(), JAVA_LANG])
            .filter(|package| !package.is_empty())
            .map(|package| format!("{package}{QUALIFIER_SEPARATOR}{written}")),
    );
    found.push(written.to_owned());
    found
}
