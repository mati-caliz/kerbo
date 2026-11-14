use super::java::{Declaration, JavaSummary};
use super::java_null_arguments::{Slot, accepts, parameter_at, project_owners};
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use crate::jvm::library::Library;
use crate::lint::java_calls::{Body, ReturnType};
use crate::lint::java_lambda_returns::{LambdaReturn, LambdaTarget};
use crate::lint::java_null_flow::NullReturn;
use crate::lint::java_nullable_fields::ReturnNullness;
use crate::lint::java_type_arguments::TypeArgument;

// Las interfaces funcionales del JDK y cuál de sus argumentos de tipo es lo que devuelven.
const LIBRARY_FUNCTIONAL_RETURNS: &[(&str, usize)] = &[
    ("java.util.function.Function", 1),
    ("java.util.function.BiFunction", 2),
    ("java.util.function.Supplier", 0),
    ("java.util.function.UnaryOperator", 0),
    ("java.util.function.BinaryOperator", 0),
    ("java.util.concurrent.Callable", 0),
    ("java.security.PrivilegedAction", 0),
    ("java.security.PrivilegedExceptionAction", 0),
];
const PARAMETRIC_NULLNESS: &str = "ParametricNullness";
const BOXED_VOID: &str = "Void";

// Dónde está escrita la interfaz funcional que recibe la lambda y qué se sabe de ella: el archivo
// que la nombra, su nombre, sus argumentos de tipo, si ese lugar es `@NullMarked`, y si la llamada
// tiene un tipo destino (y es `@NullMarked`) del que salen las variables de tipo del método.
struct FunctionalSlot<'slot> {
    written_in: &'slot JavaSummary,
    type_name: &'slot str,
    arguments: &'slot [TypeArgument],
    marked: bool,
    method_variables_marked: bool,
}

// Los `null` que devuelven lambdas cuya interfaz funcional, ya sustituida, devuelve algo no nulo:
// "'null' is returned by the method declared as @NullMarked".
pub(super) fn lambda_null_returns<'summary>(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &'summary JavaSummary,
) -> impl Iterator<Item = &'summary NullReturn> {
    summary
        .lambda_returns
        .iter()
        .filter(move |lambda| returns_non_null(types, library, summary, lambda))
        .flat_map(|lambda| &lambda.returns)
}

fn returns_non_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    lambda: &LambdaReturn,
) -> bool {
    let caller_marked = lambda
        .returns
        .first()
        .is_some_and(|returned| types.is_marked(summary, returned.marking));
    match &lambda.target {
        LambdaTarget::Declared { type_name, arguments } => functional_returns_non_null(
            types,
            library,
            &FunctionalSlot {
                written_in: summary,
                type_name,
                arguments,
                marked: caller_marked,
                method_variables_marked: false,
            },
        ),
        LambdaTarget::Argument {
            argument,
            target_typed,
        } => {
            let candidates: Vec<(&JavaSummary, &Declaration, usize)> =
                project_owners(types, library, summary, argument)
                    .iter()
                    .find_map(|owner| {
                        let found: Vec<_> = types
                            .declarations_of(owner)
                            .filter(|(_, declaration)| {
                                declaration.is_constructor == argument.is_constructor
                                    && declaration.name == argument.name
                                    && accepts(declaration, argument.arity)
                            })
                            .filter_map(|(declaring, declaration)| {
                                let (_, slot) = parameter_at(declaration, argument)?;
                                (slot == Slot::Whole).then_some((declaring, declaration, argument.index))
                            })
                            .collect();
                        (!found.is_empty()).then_some(found)
                    })
                    .unwrap_or_default();
            !candidates.is_empty()
                && candidates.iter().all(|(declaring, declaration, index)| {
                    declaration.parameters.get(*index).is_some_and(|parameter| {
                        let marked = types.is_null_marked(declaring, &declaration.owner);
                        functional_returns_non_null(
                            types,
                            library,
                            &FunctionalSlot {
                                written_in: declaring,
                                type_name: &parameter.type_name,
                                arguments: &parameter.type_arguments,
                                marked,
                                method_variables_marked: marked && caller_marked && *target_typed,
                            },
                        )
                    })
                })
        }
    }
}

fn functional_returns_non_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    slot: &FunctionalSlot<'_>,
) -> bool {
    match resolve(types, library, slot.written_in, slot.type_name) {
        Resolved::Project(interface) => project_returns_non_null(types, &interface, slot),
        Resolved::Library(interface) => LIBRARY_FUNCTIONAL_RETURNS
            .iter()
            .find(|(name, _)| *name == interface)
            .is_some_and(|(_, position)| substituted_non_null(slot, *position)),
    }
}

// La interfaz del proyecto con un solo método abstracto: si devuelve una variable de la interfaz,
// decide lo que se escribió en su lugar; si no, es no nulo bajo `@NullMarked`.
fn project_returns_non_null(types: &ProjectTypes<'_>, interface: &str, slot: &FunctionalSlot<'_>) -> bool {
    let mut abstract_methods = types
        .declarations_of(interface)
        .filter(|(_, declaration)| declaration.body == Body::Abstract && !declaration.is_constructor);
    let (Some((declaring, method)), None) = (abstract_methods.next(), abstract_methods.next()) else {
        return false;
    };
    if method.return_type != ReturnType::Reference || method.return_nullness == ReturnNullness::Nullable {
        return false;
    }
    let interface_variable = declaring
        .generics
        .types
        .iter()
        .find(|generics| generics.name == interface)
        .and_then(|generics| {
            generics
                .parameters
                .iter()
                .position(|name| *name == method.returned)
        });
    if let Some(position) = interface_variable {
        return substituted_non_null(slot, position);
    }
    let method_variable = declaring.generics.methods.iter().any(|generics| {
        generics.line == method.position.line
            && generics.column == method.position.column
            && generics.parameters.contains(&method.returned)
    });
    !method_variable
        && method.returned != BOXED_VOID
        && !method.annotations.iter().any(|name| name == PARAMETRIC_NULLNESS)
        && types.is_null_marked(declaring, &method.owner)
}

fn substituted_non_null(slot: &FunctionalSlot<'_>, position: usize) -> bool {
    match slot.arguments.get(position) {
        Some(TypeArgument::Concrete) => slot.marked,
        Some(TypeArgument::MethodVariable) => slot.method_variables_marked,
        _ => false,
    }
}
