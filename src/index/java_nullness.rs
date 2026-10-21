use std::collections::{HashMap, HashSet};

use super::java::{Declaration, JavaSummary, lint_at};
use super::java_project::IndexedSummary;
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::rules::JavaRule;

const WILDCARD_SUFFIX: &str = ".*";
const JAVA_LANG: &str = "java.lang.";
const QUALIFIER_SEPARATOR: char = '.';
const NULLABLE: &str = "Nullable";
const PROJECT_DEFAULTS: &[&str] = &["NullMarked", "NonNullApi"];

// Los tipos del proyecto por nombre simple, con el archivo que los declara (para resolver sus
// padres con los imports de ese archivo).
pub struct ProjectTypes<'summary> {
    declared: HashMap<&'summary str, Vec<(&'summary JavaSummary, &'summary [String])>>,
}

impl<'summary> ProjectTypes<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut declared: HashMap<&str, Vec<(&JavaSummary, &[String])>> = HashMap::new();
        for file in files {
            for (name, supertypes) in &file.summary.hierarchy {
                declared
                    .entry(name.as_str())
                    .or_default()
                    .push((file.summary, supertypes.as_slice()));
            }
        }
        Self { declared }
    }
}

enum Resolved {
    Project(String),
    Library(String),
}

// Calca NullableProblems para los parámetros: un método con `@Override` que redefine uno de una
// librería marcada como no nula por defecto (`@NullMarked`, `@NonNullApi`), con un parámetro sin
// anotación que en la librería no es `@Nullable`. Si el paquete del proyecto ya es no nulo por
// defecto, no hace falta anotar.
pub fn unannotated_overrides(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    declaration: &Declaration,
) -> Vec<LintFinding> {
    let project_default = summary
        .package_annotations
        .iter()
        .chain(&declaration.owner.annotations)
        .any(|annotation| PROJECT_DEFAULTS.contains(&annotation.as_str()));
    if !declaration.overrides || declaration.is_constructor || project_default || library.is_empty() {
        return Vec::new();
    }
    let Some((class, method)) = overridden(types, library, summary, declaration) else {
        return Vec::new();
    };
    let Some(label) = library.null_default(&class) else {
        return Vec::new();
    };
    declaration
        .parameters
        .iter()
        .zip(&method.parameter_annotations)
        .filter(|(parameter, inherited)| {
            !parameter.declares_nullness
                && !parameter.dereferenced
                && !inherited.iter().any(|annotation| is_nullable(annotation))
        })
        .map(|(parameter, _)| {
            lint_at(
                JavaRule::NullableProblems,
                &parameter.position,
                format!("{} {label}", parameter.name),
            )
        })
        .collect()
}

fn is_nullable(annotation: &str) -> bool {
    annotation.rsplit(QUALIFIER_SEPARATOR).next() == Some(NULLABLE)
}

type Overridden = (
    std::rc::Rc<crate::jvm::class_file::ClassFile>,
    crate::jvm::class_file::Method,
);

// El método de librería que redefine: subiendo por los padres de la clase (los del proyecto con
// los imports de su archivo) hasta encontrar una clase de librería que lo declare.
fn overridden(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    declaration: &Declaration,
) -> Option<Overridden> {
    let arity = declaration.parameters.len();
    let mut pending: Vec<Resolved> = supertypes_of(types, library, &declaration.owner.name, summary);
    let mut seen = HashSet::new();
    while let Some(next) = pending.pop() {
        match next {
            Resolved::Library(name) => {
                if let Some(found) = library.inherited_method(&name, &declaration.name, arity) {
                    return Some(found);
                }
            }
            Resolved::Project(name) => {
                if seen.insert(name.clone()) {
                    pending.extend(parents_of(types, library, &name));
                }
            }
        }
    }
    None
}

fn parents_of(types: &ProjectTypes<'_>, library: &Library, name: &str) -> Vec<Resolved> {
    types
        .declared
        .get(name)
        .into_iter()
        .flatten()
        .flat_map(|(owner, parents)| {
            parents
                .iter()
                .map(|parent| resolve(types, library, owner, parent))
        })
        .collect()
}

fn supertypes_of(
    types: &ProjectTypes<'_>,
    library: &Library,
    owner: &str,
    summary: &JavaSummary,
) -> Vec<Resolved> {
    summary
        .hierarchy
        .iter()
        .filter(|(name, _)| name == owner)
        .flat_map(|(_, parents)| parents)
        .map(|parent| resolve(types, library, summary, parent))
        .collect()
}

// Un nombre simple de padre, con los imports del archivo: el importado (de la librería si la
// tiene, si no del proyecto), uno del proyecto, uno de un import con comodín o del mismo paquete
// que esté en la librería, o uno de `java.lang`.
fn resolve(types: &ProjectTypes<'_>, library: &Library, summary: &JavaSummary, simple: &str) -> Resolved {
    let in_project = types.declared.contains_key(simple);
    if let Some(imported) = summary
        .imports
        .iter()
        .find(|path| path.rsplit(QUALIFIER_SEPARATOR).next() == Some(simple))
    {
        return if in_project && !library.has_class(imported) {
            Resolved::Project(simple.to_owned())
        } else {
            Resolved::Library(imported.clone())
        };
    }
    if in_project {
        return Resolved::Project(simple.to_owned());
    }
    summary
        .imports
        .iter()
        .filter_map(|path| path.strip_suffix(WILDCARD_SUFFIX))
        .chain(std::iter::once(summary.package.as_str()))
        .map(|package| format!("{package}{QUALIFIER_SEPARATOR}{simple}"))
        .find(|candidate| library.has_class(candidate))
        .map_or_else(
            || Resolved::Library(format!("{JAVA_LANG}{simple}")),
            Resolved::Library,
        )
}
