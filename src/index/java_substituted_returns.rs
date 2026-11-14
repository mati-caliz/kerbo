use super::java_null_arguments::{accepts, declares_method_variable};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage};
use crate::jvm::generics::{Generic, class_signature, method_signature};
use crate::jvm::library::Library;
use crate::lint::java_chains::ChainStep;

// Si el paso devuelve una variable de la clase que el receptor declaró `@Nullable` en ese lugar
// (`Function<String, @Nullable Integer> function` en `function.apply(x)`, `Queue<@Nullable E>` en
// `peek()`). Si el método lo declara una clase de arriba, sus variables tienen que llamarse igual.
pub(super) fn returns_nullable_argument(
    types: &ProjectTypes<'_>,
    library: &Library,
    current: &Resolved,
    (step, positions): (&ChainStep, &[usize]),
) -> bool {
    if positions.is_empty() {
        return false;
    }
    match current {
        Resolved::Project(owner) => project_returns(types, library, owner, (step, positions)),
        Resolved::Library(class) => library_returns(library, class, (step, positions)),
    }
}

fn project_returns(
    types: &ProjectTypes<'_>,
    library: &Library,
    owner: &str,
    (step, positions): (&ChainStep, &[usize]),
) -> bool {
    let Some(variables) = types.declared_in(owner).find_map(|(summary, _)| {
        summary
            .generics
            .types
            .iter()
            .find(|generics| generics.name == owner)
            .map(|generics| &generics.parameters)
    }) else {
        return false;
    };
    let candidates = project_lineage(types, library, owner).iter().find_map(|class| {
        let found: Vec<_> = types
            .declarations_of(class)
            .filter(|(_, declaration)| {
                !declaration.is_constructor
                    && declaration.name == step.name
                    && accepts(declaration, step.arity)
            })
            .collect();
        (!found.is_empty()).then_some(found)
    });
    candidates.is_some_and(|candidates| {
        candidates.iter().all(|(declaring, declaration)| {
            let same_variables = declaring
                .generics
                .types
                .iter()
                .find(|generics| generics.name == declaration.owner.name)
                .is_some_and(|generics| generics.parameters == *variables);
            same_variables
                && !declares_method_variable(declaring, declaration, &declaration.returned)
                && variables
                    .iter()
                    .position(|name| *name == declaration.returned)
                    .is_some_and(|position| positions.contains(&position))
        })
    })
}

fn library_returns(library: &Library, class: &str, (step, positions): (&ChainStep, &[usize])) -> bool {
    let variables = |found: &crate::jvm::class_file::ClassFile| {
        found
            .signature
            .as_deref()
            .and_then(class_signature)
            .map(|signature| signature.parameters)
    };
    let Some(receiver_variables) = library.class_or_jdk(class).and_then(|found| variables(&found)) else {
        return false;
    };
    let Some((declaring, methods)) = library.declared_overloads(class, &step.name, step.arity) else {
        return false;
    };
    variables(&declaring).as_ref() == Some(&receiver_variables)
        && methods.iter().all(|method| {
            method
                .signature
                .as_deref()
                .and_then(method_signature)
                .is_some_and(|signature| match &signature.returned {
                    Generic::Variable(name) => {
                        !signature.parameters.contains(name)
                            && receiver_variables
                                .iter()
                                .position(|variable| variable == name)
                                .is_some_and(|position| positions.contains(&position))
                    }
                    Generic::Class(..) | Generic::Other => false,
                })
        })
}
