use std::collections::HashSet;

use super::java::{JavaSummary, Position, lint_at};
use super::java_jdk::{CLOSEABLE, NOT_RESOURCES, STREAM_HOLDERS, returned_by};
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const AUTO_CLOSEABLE: &str = "java.lang.AutoCloseable";
const TRY_WITH_RESOURCES_RELEASE: u32 = 7;

// Dónde se sabe qué tipo tiene un paso de la cadena: en el proyecto (por nombre simple), en los
// jars, o en el JDK (que no viene en el classpath).
enum Place {
    Project(String),
    Library(String),
}

// Calca AutoCloseableResource: una llamada que crea algo que se cierra (por los jars, por el tipo
// que declara un método del proyecto o por la tabla del JDK) y que no va a un try con recursos.
pub fn unclosed_resources(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    if library
        .release()
        .is_some_and(|release| release < TRY_WITH_RESOURCES_RELEASE)
    {
        return Vec::new();
    }
    summary
        .resources
        .iter()
        .filter_map(|candidate| {
            let created = created_class(types, library, summary, &candidate.origin)?;
            (is_resource(library, &created) || holds_a_file(types, library, summary, &candidate.origin)).then(
                || {
                    let simple = created.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(&created);
                    lint_at(
                        JavaRule::AutoCloseableResource,
                        &Position {
                            line: candidate.line,
                            column: candidate.column,
                        },
                        simple.to_owned(),
                    )
                },
            )
        })
        .collect()
}

// La clase que devuelve el último paso de la cadena, siguiendo cada paso por el proyecto, los jars
// o la tabla del JDK.
fn created_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Option<String> {
    let (ChainRoot::Instance(root) | ChainRoot::Class(root)) = &chain.root;
    let mut place = match resolve(types, library, summary, root) {
        Resolved::Project(name) => Place::Project(name),
        Resolved::Library(name) => Place::Library(name),
    };
    for step in &chain.steps {
        place = match place {
            Place::Project(owner) => {
                if let Some(next) = declared_return(types, library, &owner, step) {
                    next
                } else {
                    Place::Library(inherited_return(
                        library,
                        &library_ancestors(types, library, &owner),
                        &step.name,
                        step.arity,
                    )?)
                }
            }
            Place::Library(class) => {
                if library.has_class(&class) {
                    Place::Library(inherited_return(library, &[class], &step.name, step.arity)?)
                } else {
                    Place::Library(returned_by(&class, &step.name)?.to_owned())
                }
            }
        };
    }
    match place {
        Place::Library(class) => Some(class),
        Place::Project(_) => None,
    }
}

// El tipo que declara como retorno el método del proyecto, resuelto con los imports de su archivo.
fn declared_return(
    types: &ProjectTypes<'_>,
    library: &Library,
    owner: &str,
    step: &ChainStep,
) -> Option<Place> {
    let (summary, declaration) = types.declarations_of(owner).find(|(_, declaration)| {
        declaration.name == step.name
            && declaration.parameters.len() == step.arity
            && !declaration.is_constructor
    })?;
    Some(match resolve(types, library, summary, &declaration.returned) {
        Resolved::Project(name) => Place::Project(name),
        Resolved::Library(name) => Place::Library(name),
    })
}

fn inherited_return(library: &Library, classes: &[String], method: &str, arity: usize) -> Option<String> {
    let candidates = classes
        .iter()
        .map(|class| library.overloads(class, method, arity))
        .find(|found| !found.is_empty())?;
    let returned = candidates.first()?.return_class();
    candidates
        .iter()
        .all(|candidate| candidate.return_class() == returned)
        .then_some(returned)
        .flatten()
}

// `Files.lines(ruta)` y compañía: un `Stream` que IntelliJ pide cerrar.
fn holds_a_file(types: &ProjectTypes<'_>, library: &Library, summary: &JavaSummary, chain: &Chain) -> bool {
    let (holder, methods) = STREAM_HOLDERS;
    let (ChainRoot::Instance(root) | ChainRoot::Class(root)) = &chain.root;
    matches!(chain.steps.as_slice(), [step] if methods.contains(&step.name.as_str()))
        && resolve(types, library, summary, root) == Resolved::Library(holder.to_owned())
}

// Si la clase toma algo del sistema que hay que cerrar. Con el JDK, como IntelliJ: cualquier
// `AutoCloseable` que no herede de uno de los ignorados. Sin él, por la tabla.
fn is_resource(library: &Library, class: &str) -> bool {
    if library.class_or_jdk(AUTO_CLOSEABLE).is_none() {
        return is_listed_resource(library, class);
    }
    let ancestors = ancestors(library, class);
    ancestors.iter().any(|ancestor| ancestor == AUTO_CLOSEABLE)
        && !ancestors
            .iter()
            .any(|ancestor| NOT_RESOURCES.contains(&ancestor.as_str()))
}

fn ancestors(library: &Library, class: &str) -> Vec<String> {
    let mut pending = vec![class.to_owned()];
    let mut seen = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        if let Some(found) = library.class_or_jdk(&current) {
            pending.extend(found.interfaces.iter().cloned());
            pending.extend(found.superclass.clone());
        }
        seen.push(current);
    }
    seen
}

fn is_listed_resource(library: &Library, class: &str) -> bool {
    let mut pending = vec![class.to_owned()];
    let mut seen = HashSet::new();
    while let Some(current) = pending.pop() {
        if NOT_RESOURCES.contains(&current.as_str()) {
            return false;
        }
        if CLOSEABLE.contains(&current.as_str()) {
            return true;
        }
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(found) = library.class(&current) {
            pending.extend(found.interfaces.iter().cloned());
            pending.extend(found.superclass.clone());
        }
    }
    false
}
