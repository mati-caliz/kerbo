use std::collections::HashSet;

use super::java::{JavaSummary, Position, lint_at};
use super::java_jdk::{CLOSEABLE, NOT_RESOURCES, returned_by};
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';

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
    summary
        .resources
        .iter()
        .filter_map(|candidate| {
            let created = created_class(types, library, summary, &candidate.origin)?;
            is_resource(library, &created).then(|| {
                let simple = created.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(&created);
                lint_at(
                    JavaRule::AutoCloseableResource,
                    &Position {
                        line: candidate.line,
                        column: candidate.column,
                    },
                    simple.to_owned(),
                )
            })
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

// Si la clase toma algo del sistema que hay que cerrar: está en la tabla del JDK o hereda (por los
// jars) de algo que está.
fn is_resource(library: &Library, class: &str) -> bool {
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
