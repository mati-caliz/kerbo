use super::java::JavaSummary;
use super::java_dereference::report;
use super::java_inferred_arguments::project_candidates;
use super::java_nullness::{ProjectTypes, Resolved, resolve};
use crate::jvm::generics::{Generic, method_signature};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_nested_arguments::ReferenceArgument;
use crate::lint::java_type_arguments::TypeArgument;

// Las interfaces funcionales del JDK y, para cada parámetro de su método, qué argumento de tipo
// es su tipo.
const FUNCTIONAL_PARAMETERS: &[(&str, &[usize])] = &[
    ("java.util.function.Function", &[0]),
    ("java.util.function.BiFunction", &[0, 1]),
    ("java.util.function.UnaryOperator", &[0]),
    ("java.util.function.BinaryOperator", &[0, 0]),
    ("java.util.function.Consumer", &[0]),
    ("java.util.function.BiConsumer", &[0, 1]),
    ("java.util.function.Predicate", &[0]),
    ("java.util.function.BiPredicate", &[0, 1]),
];

// Calca el "Passing an argument to the method reference requires unboxing which may produce
// 'NullPointerException'": una referencia a un método estático con parámetros primitivos
// (`Integer::sum`) pasada donde la interfaz funcional le da algo `@Nullable`
// (`BiFunction<? super V, ? super @Nullable V, ...>` en `RangeMap.merge`).
pub(super) fn unboxed_references(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    summary
        .references
        .iter()
        .filter(|reference| unboxes(types, library, summary, reference))
        .map(|reference| {
            report(
                reference.argument.line,
                reference.argument.column,
                &reference.method,
            )
        })
        .collect()
}

fn unboxes(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    reference: &ReferenceArgument,
) -> bool {
    let candidates = project_candidates(types, library, summary, &reference.argument);
    !candidates.is_empty()
        && candidates.iter().all(|(declaring, _, parameter)| {
            let Resolved::Library(interface) = resolve(types, library, declaring, &parameter.type_name)
            else {
                return false;
            };
            let Some((_, slots)) = FUNCTIONAL_PARAMETERS.iter().find(|(name, _)| *name == interface) else {
                return false;
            };
            let primitive = primitive_parameters(types, library, summary, (reference, slots.len()));
            slots.iter().enumerate().any(|(position, slot)| {
                parameter.type_arguments.get(*slot) == Some(&TypeArgument::Nullable)
                    && primitive.get(position) == Some(&true)
            })
        })
}

// Cuáles de los parámetros del método referido son primitivos, si todas sus sobrecargas con esa
// cantidad coinciden.
fn primitive_parameters(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (reference, arity): (&ReferenceArgument, usize),
) -> Vec<bool> {
    let found: Vec<Vec<bool>> = match resolve(types, library, summary, &reference.owner) {
        Resolved::Library(class) => library
            .overloads(&class, &reference.method, arity)
            .iter()
            .filter_map(|method| method_signature(&method.descriptor))
            .map(|signature| {
                signature
                    .arguments
                    .iter()
                    .map(|argument| *argument == Generic::Other)
                    .collect()
            })
            .collect(),
        Resolved::Project(owner) => types
            .declarations_of(&owner)
            .filter(|(_, declaration)| {
                !declaration.is_constructor
                    && declaration.name == reference.method
                    && declaration.parameters.len() == arity
            })
            .map(|(_, declaration)| {
                declaration
                    .parameters
                    .iter()
                    .map(|parameter| !parameter.accepts_null)
                    .collect()
            })
            .collect(),
    };
    let Some(first) = found.first() else {
        return Vec::new();
    };
    if found.iter().all(|other| other == first) {
        first.clone()
    } else {
        Vec::new()
    }
}
