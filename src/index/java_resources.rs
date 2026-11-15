use std::collections::HashSet;

use super::java::{JavaSummary, Position, lint_at};
use super::java_jdk::{CLOSEABLE, NOT_RESOURCES, STREAM_HOLDERS, returned_by};
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, resolve};
use super::java_resource_owners::{creates_a_resource, keeps_its_type};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_resources::ResourceOrigin;
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const AUTO_CLOSEABLE: &str = "java.lang.AutoCloseable";
const TRY_WITH_RESOURCES_RELEASE: u32 = 7;
const BASE_STREAM: &str = "java.util.stream.BaseStream";
const IGNORED_METHODS: &[(&str, &str)] = &[
    ("java.util.Formatter", "format"),
    ("java.io.Writer", "append"),
    ("com.google.common.base.Preconditions", "checkNotNull"),
    ("org.hibernate.Session", "close"),
    ("java.io.PrintWriter", "printf"),
    ("java.io.PrintStream", "printf"),
    ("java.lang.foreign.Arena", "ofAuto"),
];

// Dónde se sabe qué tipo tiene un paso de la cadena: en el proyecto (por nombre simple), en los
// jars, o en el JDK (que no viene en el classpath).
#[derive(PartialEq, Eq)]
pub(super) enum Place {
    Project(String),
    Library(String),
}

// Calca AutoCloseableResource: una llamada o un `new` que crea algo que se cierra (por los jars, por
// el tipo que declara un método del proyecto o por la tabla del JDK) y que no va a un try con
// recursos.
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
        .filter(|candidate| {
            !candidate
                .owners
                .iter()
                .any(|owner| is_closeable(types, library, summary, owner))
                && !candidate
                    .receiver
                    .as_ref()
                    .is_some_and(|receiver| creates_a_resource(types, library, summary, receiver))
        })
        .filter(|candidate| !candidate.kept_at_end || !keeps_its_type(types, library, summary, candidate))
        .filter_map(|candidate| {
            let created = unclosed_class(types, library, summary, &candidate.origin)?;
            let simple = created.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(&created);
            Some(lint_at(
                JavaRule::AutoCloseableResource,
                &Position {
                    line: candidate.line,
                    column: candidate.column,
                },
                simple.to_owned(),
            ))
        })
        .collect()
}

// La clase de lo creado, si es algo que hay que cerrar.
pub(super) fn unclosed_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    origin: &ResourceOrigin,
) -> Option<String> {
    match origin {
        ResourceOrigin::Call(chain) => {
            if is_ignored_call(types, library, summary, chain) {
                return None;
            }
            match created_place(types, library, summary, chain)? {
                Place::Library(created) => (is_resource(library, &created)
                    || holds_a_file(types, library, summary, chain))
                .then_some(created),
                Place::Project(created) => {
                    is_project_resource(library, &library_ancestors(types, library, &created))
                        .then_some(created)
                }
            }
        }
        ResourceOrigin::Construction(class) => match resolve(types, library, summary, class) {
            Resolved::Library(created) => is_resource(library, &created).then_some(created),
            Resolved::Project(created) => {
                is_project_resource(library, &library_ancestors(types, library, &created)).then_some(created)
            }
        },
    }
}

// Si lo que da el argumento o el receptor es `AutoCloseable`, aunque sea de los ignorados: el
// recurso nuevo puede quedarse con él y IntelliJ no sigue a ninguno de los dos.
fn is_closeable(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    owner: &ResourceOrigin,
) -> bool {
    let inherited = match owner {
        ResourceOrigin::Call(chain) => match created_place(types, library, summary, chain) {
            Some(Place::Library(created)) => ancestors(library, &created),
            Some(Place::Project(created)) => library_ancestors(types, library, &created)
                .iter()
                .flat_map(|ancestor| ancestors(library, ancestor))
                .collect(),
            None => Vec::new(),
        },
        ResourceOrigin::Construction(class) => match resolve(types, library, summary, class) {
            Resolved::Library(created) => ancestors(library, &created),
            Resolved::Project(created) => library_ancestors(types, library, &created)
                .iter()
                .flat_map(|ancestor| ancestors(library, ancestor))
                .collect(),
        },
    };
    inherited.iter().any(|class| {
        class == AUTO_CLOSEABLE
            || CLOSEABLE.contains(&class.as_str())
            || NOT_RESOURCES.contains(&class.as_str())
    })
}

// Una clase del proyecto se cierra si hereda de un recurso de una librería y de ninguno de los
// ignorados.
fn is_project_resource(library: &Library, inherited: &[String]) -> bool {
    inherited.iter().any(|class| is_resource(library, class))
        && !inherited.iter().any(|class| {
            ancestors(library, class)
                .iter()
                .any(|ancestor| NOT_RESOURCES.contains(&ancestor.as_str()))
        })
}

// Lo que devuelve el último paso de la cadena, siguiendo cada paso por el proyecto, los jars o la
// tabla del JDK. Una llamada sin receptor que la clase no declara puede venir de un `import static`.
pub(super) fn created_place(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Option<Place> {
    let (ChainRoot::Instance(root) | ChainRoot::Class(root)) = &chain.root;
    let mut place = place_of(resolve(types, library, summary, root));
    for (index, step) in chain.steps.iter().enumerate() {
        let next = next_place(types, library, place, step).or_else(|| {
            let imported = (index == 0)
                .then(|| imported_owner(types, library, summary, &step.name))
                .flatten()?;
            next_place(types, library, Place::Project(imported), step)
        });
        place = next?;
    }
    Some(place)
}

fn next_place(types: &ProjectTypes<'_>, library: &Library, place: Place, step: &ChainStep) -> Option<Place> {
    match place {
        Place::Project(owner) => declared_return(types, library, &owner, step).or_else(|| {
            inherited_return(
                library,
                &library_ancestors(types, library, &owner),
                &step.name,
                step.arity,
            )
            .map(Place::Library)
        }),
        Place::Library(class) => {
            let returned = if library.has_class(&class) {
                inherited_return(library, std::slice::from_ref(&class), &step.name, step.arity)?
            } else {
                returned_by(&class, &step.name)?.to_owned()
            };
            Some(Place::Library(if returned == BASE_STREAM {
                class
            } else {
                returned
            }))
        }
    }
}

fn place_of(resolved: Resolved) -> Place {
    match resolved {
        Resolved::Project(name) => Place::Project(name),
        Resolved::Library(name) => Place::Library(name),
    }
}

// Los métodos que IntelliJ no cuenta como creación de un recurso (`MethodMatcher` por defecto):
// devuelven el mismo objeto que reciben (`writer.append(...)`) o lo chequean.
fn is_ignored_call(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> bool {
    let Some((last, before)) = chain.steps.split_last() else {
        return false;
    };
    let receiver = Chain {
        steps: before.to_vec(),
        ..chain.clone()
    };
    let Some(place) = created_place(types, library, summary, &receiver) else {
        return false;
    };
    let lineage = match place {
        Place::Library(class) => ancestors(library, &class),
        Place::Project(class) => library_ancestors(types, library, &class)
            .iter()
            .flat_map(|ancestor| ancestors(library, ancestor))
            .chain(std::iter::once(class))
            .collect(),
    };
    IGNORED_METHODS.iter().any(|(owner, method)| {
        *method == last.name
            && lineage.iter().any(|class| {
                class == owner || owner.rsplit(QUALIFIER_SEPARATOR).next() == Some(class.as_str())
            })
    })
}

fn imported_owner(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    method: &str,
) -> Option<String> {
    let owner = summary.static_imports.iter().find_map(|path| {
        let (owner, member) = path.rsplit_once(QUALIFIER_SEPARATOR)?;
        (member == method).then_some(owner)
    })?;
    match resolve(types, library, summary, owner) {
        Resolved::Project(name) => Some(name),
        Resolved::Library(_) => None,
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
