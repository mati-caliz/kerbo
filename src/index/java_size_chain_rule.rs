use super::java::JavaSummary;
use super::java_nullness::Resolved;
use super::java_project::IndexedSummary;
use super::java_throw_lookup::ThrowLookup;
use crate::jvm::library::Library;
use crate::lint::java_size_is_empty::has_is_empty_type;
use crate::lint::java_throws::ThrowOrigin;
use crate::lint::{LintFinding, LintKind};
use crate::rules::JavaRule;

const SEPARATOR: char = '.';
const WITH_IS_EMPTY_ROOTS: [&str; 3] = ["java.util.Collection", "java.util.Map", "java.lang.String"];

// `SizeReplaceableByIsEmpty` sobre lo que devuelve una cadena de llamadas (`a.getItems().size() >
// 0`): vale si la cadena se resuelve, con los tipos del proyecto y de las dependencias, a algo con
// isEmpty(). El índice guarda el tipo devuelto sin los `[]`, así que si algún método del proyecto
// con el nombre del último paso devuelve un arreglo, no se avisa.
pub fn chained_sizes(
    lookup: &ThrowLookup<'_, '_>,
    library: &Library,
    files: &[IndexedSummary<'_>],
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    summary
        .size_candidates
        .iter()
        .filter(|candidate| !returns_an_array(files, &candidate.origin))
        .filter(|candidate| {
            lookup
                .returned_types(summary, &candidate.origin)
                .is_some_and(|types| {
                    !types.is_empty() && types.iter().all(|typed| has_is_empty(library, &typed.class))
                })
        })
        .map(|candidate| LintFinding {
            kind: LintKind::Java(JavaRule::SizeReplaceableByIsEmpty),
            line: candidate.line,
            column: candidate.column,
            subject: candidate.replacement.clone(),
            fix: None,
        })
        .collect()
}

fn returns_an_array(files: &[IndexedSummary<'_>], origin: &ThrowOrigin) -> bool {
    let ThrowOrigin::Call(_, steps) = origin else {
        return false;
    };
    steps.last().is_some_and(|last| {
        files
            .iter()
            .any(|file| file.summary.array_methods.contains(&last.name))
    })
}

fn has_is_empty(library: &Library, class: &Resolved) -> bool {
    match class {
        Resolved::Project(name) => has_is_empty_type(name),
        Resolved::Library(name) => {
            let simple = name.rsplit(SEPARATOR).next().unwrap_or(name);
            has_is_empty_type(simple)
                || library
                    .supertypes(name)
                    .iter()
                    .any(|ancestor| WITH_IS_EMPTY_ROOTS.contains(&ancestor.as_str()))
        }
    }
}
