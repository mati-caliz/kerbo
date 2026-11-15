use super::java::{Declaration, JavaSummary};
use super::java_library_arguments::declared_member_type;
use super::java_null_arguments::{accepts, declares_method_variable};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use super::java_throw_project::generics_in;
use crate::jvm::library::Library;
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_generics::TypeGenerics;
use crate::lint::java_receiver_arguments::InheritedMember;
use crate::lint::java_receivers::simple_type_name;

const MAX_LINEAGE_DEPTH: usize = 16;

// La clase que encierra una llamada sin receptor dentro de una clase anidada o anónima y que
// declara (ella o una de arriba en el proyecto) el método llamado: la más cercana.
pub(super) fn enclosing_owner(types: &ProjectTypes<'_>, library: &Library, chain: &Chain) -> Option<String> {
    let first = chain.steps.first()?;
    chain
        .enclosing
        .iter()
        .find(|class| {
            project_lineage(types, library, class).iter().any(|ancestor| {
                types
                    .declarations_of(ancestor)
                    .any(|(_, declaration)| !declaration.is_constructor && declaration.name == first.name)
            })
        })
        .cloned()
}

// Una llamada sin receptor a un método heredado que devuelve una variable de tipo de su clase
// (`e0()` de `AbstractContainerTester<C, E extends @Nullable Object>`) que, siguiendo los padres
// genéricos desde la clase que llama, es una variable suya con límite `@Nullable`, en código
// `@NullMarked`: IntelliJ la da por posiblemente null. Si el método sale de la clase más cercana de
// varias, que puede ser una anónima con sus propios argumentos de tipo, no se sigue.
pub(super) fn returns_nullable_variable(types: &ProjectTypes<'_>, library: &Library, chain: &Chain) -> bool {
    let ([step], ChainRoot::Class(innermost)) = (chain.steps.as_slice(), &chain.root) else {
        return false;
    };
    let caller = enclosing_owner(types, library, chain).unwrap_or_else(|| innermost.clone());
    if chain.enclosing.first() == Some(&caller) {
        return false;
    }
    let Some((declaring, declaration)) = inherited_declaration(types, library, &caller, step) else {
        return false;
    };
    if declares_method_variable(declaring, declaration, &declaration.returned) {
        return false;
    }
    let caller_marked = types
        .declarations_of(&caller)
        .any(|(file, declared)| types.is_null_marked(file, &declared.owner));
    caller_marked
        && bound_in(
            types,
            &caller,
            (&declaration.owner.name, &declaration.returned),
            0,
        )
        .is_some_and(|bound| {
            generics_of(types, &caller).is_some_and(|generics| generics.nullable_parameters.contains(&bound))
        })
}

// El tipo que toma, visto desde `class`, la variable de tipo `variable` que declara `target`: su
// nombre simple, o la variable de `class` que la reemplaza.
fn bound_in(
    types: &ProjectTypes<'_>,
    class: &str,
    (target, variable): (&str, &str),
    depth: usize,
) -> Option<String> {
    if class == target {
        return Some(variable.to_owned());
    }
    if depth >= MAX_LINEAGE_DEPTH {
        return None;
    }
    generics_of(types, class)?.parents.iter().find_map(|parent| {
        let inner = bound_in(
            types,
            &parent.written,
            (target, variable),
            depth.saturating_add(1),
        )?;
        let position = generics_of(types, &parent.written)
            .and_then(|declared| declared.parameters.iter().position(|name| *name == inner));
        let Some(position) = position else {
            return Some(inner);
        };
        parent
            .arguments
            .get(position)
            .map(|argument| simple_type_name(argument))
    })
}

fn generics_of<'summary>(types: &ProjectTypes<'summary>, class: &str) -> Option<&'summary TypeGenerics> {
    types
        .declared_in(class)
        .find_map(|(summary, _)| generics_in(summary, class))
}

// El método que llama el paso, en la clase o en las de arriba en el proyecto, si es uno solo.
fn inherited_declaration<'summary>(
    types: &ProjectTypes<'summary>,
    library: &Library,
    caller: &str,
    step: &ChainStep,
) -> Option<(&'summary JavaSummary, &'summary Declaration)> {
    let found: Vec<_> = project_lineage(types, library, caller).iter().find_map(|class| {
        let found: Vec<_> = types
            .declarations_of(class)
            .filter(|(_, declaration)| {
                !declaration.is_constructor
                    && declaration.name == step.name
                    && accepts(declaration, step.arity)
            })
            .collect();
        (!found.is_empty()).then_some(found)
    })?;
    match found.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

// La clase del proyecto de un receptor heredado: el tipo del campo o lo que devuelve el getter que
// declara la clase más cercana de las de arriba, con sus variables de tipo reemplazadas por lo que
// les pasan los padres genéricos (`getSubjectGenerator()` devuelve el `G` de `AbstractTester<G>`).
pub(super) fn inherited_project_class(
    types: &ProjectTypes<'_>,
    library: &Library,
    member: &InheritedMember,
) -> Option<String> {
    let (scope, class, declaring, written) = member.scope.iter().find_map(|scope| {
        project_lineage(types, library, scope)
            .into_iter()
            .find_map(|class| {
                let (declaring, written) = declared_member_type(types, &class, member)?;
                Some((scope, class, declaring, written))
            })
    })?;
    let is_variable =
        generics_in(declaring, &class).is_some_and(|generics| generics.parameters.contains(&written));
    let named = if is_variable {
        bound_in(types, scope, (&class, &written), 0)?
    } else {
        simple_type_name(&written)
    };
    match resolve(types, library, declaring, &named) {
        Resolved::Project(found) => Some(found),
        Resolved::Library(_) => None,
    }
}
