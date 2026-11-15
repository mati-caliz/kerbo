use std::collections::HashSet;

use super::java::JavaSummary;
use super::java_nullness::ProjectTypes;
use crate::jvm::library::Library;
use crate::lint::java_import_shadows::Enclosing;
use crate::lint::{LintFinding, LintKind};

const SEPARATOR: char = '.';
const WILDCARD_SUFFIX: &str = ".*";
const JAVA_LANG: &str = "java.lang";

// Lo que hace falta para buscar un tipo miembro desde un archivo.
struct Search<'context, 'summary> {
    types: &'context ProjectTypes<'summary>,
    library: &'context Library,
    summary: &'context JavaSummary,
    member: &'context str,
}

// `UnnecessaryImport` de PMD con tipos resueltos: el import de un tipo anidado no hace falta si en
// cada uso el nombre ya llega como tipo miembro de una clase que lo encierra o de sus padres (del
// proyecto, de las dependencias o del JDK). Los resúmenes sólo traen estos casos imitando a PMD.
pub fn shadowed_imports(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    summary
        .import_shadows
        .iter()
        .filter(|shadow| {
            let search = Search {
                types,
                library,
                summary,
                member: shadow
                    .imported
                    .rsplit(SEPARATOR)
                    .next()
                    .unwrap_or(&shadow.imported),
            };
            shadow.usages.iter().all(|usage| search.shadowed_in(usage))
        })
        .map(|shadow| LintFinding {
            kind: LintKind::UnusedImport,
            line: shadow.line,
            column: shadow.column,
            subject: shadow.imported.clone(),
            fix: None,
        })
        .collect()
}

impl Search<'_, '_> {
    // De la clase más cercana a la de afuera: la primera que trae el tipo miembro tapa el import; pero
    // si una tiene un supertipo que PMD no puede resolver como está escrito (un tipo anidado
    // heredado, sin import), PMD no sabe qué trae y lo da por usado.
    fn shadowed_in(&self, usage: &[Enclosing]) -> bool {
        for enclosing in usage {
            if enclosing
                .supertypes
                .iter()
                .any(|supertype| !self.resolvable_as_written(supertype))
            {
                return false;
            }
            if self.provides(enclosing) {
                return true;
            }
        }
        false
    }

    fn resolvable_as_written(&self, written: &str) -> bool {
        let summary = self.summary;
        let declared_here = summary.hierarchy.iter().any(|(name, _)| name == written);
        let imported = summary
            .imports
            .iter()
            .any(|import| import.rsplit(SEPARATOR).next() == Some(written));
        let top_level_in_package = self
            .types
            .declaring_file(written, summary)
            .is_some_and(|declaring| {
                declaring.package == summary.package
                    && !declaring.nested_types.iter().any(|(_, inner)| inner == written)
            });
        declared_here
            || imported
            || top_level_in_package
            || qualified_candidates(summary, written)
                .iter()
                .any(|qualified| self.library.class_or_jdk(qualified).is_some())
    }

    fn provides(&self, enclosing: &Enclosing) -> bool {
        let declares_it = enclosing.name.as_deref().is_some_and(|name| {
            self.summary
                .nested_types
                .iter()
                .any(|(outer, inner)| outer == name && inner == self.member)
        });
        let mut visited = HashSet::new();
        declares_it
            || enclosing
                .supertypes
                .iter()
                .any(|supertype| self.inherits_member(self.summary, supertype, &mut visited))
    }

    fn inherits_member(&self, from: &JavaSummary, type_name: &str, visited: &mut HashSet<String>) -> bool {
        if !visited.insert(type_name.to_owned()) {
            return false;
        }
        if let Some(declaring) = self.types.declaring_file(type_name, from) {
            if declaring
                .nested_types
                .iter()
                .any(|(outer, inner)| outer == type_name && inner == self.member)
            {
                return true;
            }
            let parents: Vec<String> = declaring
                .hierarchy
                .iter()
                .filter(|(name, _)| name == type_name)
                .flat_map(|(_, parents)| parents.iter().cloned())
                .collect();
            return parents
                .iter()
                .any(|parent| self.inherits_member(declaring, parent, visited));
        }
        qualified_candidates(from, type_name)
            .into_iter()
            .find(|qualified| self.library.class_or_jdk(qualified).is_some())
            .is_some_and(|qualified| {
                self.library.supertypes(&qualified).iter().any(|ancestor| {
                    self.library
                        .has_class(&format!("{ancestor}{SEPARATOR}{}", self.member))
                })
            })
    }
}

fn qualified_candidates(summary: &JavaSummary, simple: &str) -> Vec<String> {
    let mut found: Vec<String> = summary
        .imports
        .iter()
        .filter(|import| import.rsplit(SEPARATOR).next() == Some(simple))
        .cloned()
        .collect();
    found.extend(
        summary
            .imports
            .iter()
            .filter_map(|import| import.strip_suffix(WILDCARD_SUFFIX))
            .chain([summary.package.as_str(), JAVA_LANG])
            .filter(|package| !package.is_empty())
            .map(|package| format!("{package}{SEPARATOR}{simple}")),
    );
    found
}
