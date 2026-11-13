use super::java::{
    Declaration, Dereferenced, JavaSummary, NullInference, Parameter, ParameterNullness, Position, lint_at,
};
use super::java_dereference::may_return_null;
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body};
use crate::lint::java_null_flow::{NullFlow, NullOrigin};
use crate::lint::java_null_inference::{ARRAY_SUFFIX, NullArgument};
use crate::lint::java_receivers::{Receiver, simple_type_name};
use crate::rules::JavaRule;

// Los parámetros del JDK que las anotaciones externas de IntelliJ marcan `@NotNull`: clase (vale
// también para las que la heredan), método, cantidad de argumentos, posición y nombre.
const LIBRARY_NON_NULL: &[(&str, &str, usize, usize, &str)] = &[
    ("java.util.concurrent.Future", "get", 2, 1, "unit"),
    ("java.util.Collection", "addAll", 1, 0, "c"),
    ("java.util.Collection", "removeAll", 1, 0, "c"),
    ("java.util.Collection", "retainAll", 1, 0, "c"),
    ("java.util.Collection", "containsAll", 1, 0, "c"),
    ("java.util.Map", "putAll", 1, 0, "m"),
];

// Calca el "Passing 'null' argument to parameter annotated as non-null" de DataFlowIssue con lo que
// IntelliJ infiere sin anotaciones: un parámetro que el método desreferencia en todos los caminos es
// no nulo, si nadie puede redefinir el método. Con sobrecargas, todas tienen que coincidir. También
// el "Argument might be null": una variable asignada desde una llamada que puede devolver null y
// pasada así a ese parámetro.
pub fn null_arguments(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    let possibly_null = summary
        .null_flows
        .iter()
        .filter(|flow| !flow.arguments.is_empty())
        .filter(|flow| reaches(types, library, summary, flow))
        .flat_map(|flow| &flow.arguments);
    summary
        .null_arguments
        .iter()
        .chain(possibly_null)
        .filter_map(|argument| {
            let parameter = non_null_parameter(types, library, summary, argument)?;
            Some(lint_at(
                JavaRule::NullArgument,
                &Position {
                    line: argument.line,
                    column: argument.column,
                },
                parameter,
            ))
        })
        .collect()
}

// Si la variable del flujo puede llegar null a sus usos: la llamada que le dio el valor puede
// devolver null y ninguna llamada anterior a la que se pasó exige un parámetro no nulo.
pub(super) fn reaches(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    flow: &NullFlow,
) -> bool {
    let origin_may_be_null = match &flow.origin {
        NullOrigin::Call(chain) => may_return_null(types, library, summary, chain),
        NullOrigin::NullableField => true,
    };
    origin_may_be_null
        && flow
            .passed_before
            .iter()
            .all(|argument| non_null_parameter(types, library, summary, argument).is_none())
}

pub(super) fn non_null_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Option<String> {
    let owners: Vec<String> = match &argument.receiver {
        Receiver::Type(written) => match resolve(types, library, summary, written) {
            Resolved::Project(owner) if argument.is_constructor => vec![owner],
            Resolved::Project(owner) => project_lineage(types, library, &owner),
            Resolved::Library(class) => return library_non_null(library, &class, argument),
        },
        Receiver::Scope(enclosing) => enclosing
            .iter()
            .flat_map(|owner| project_lineage(types, library, owner))
            .collect(),
        Receiver::Unknown => return None,
    };
    let candidates = owners.iter().find_map(|owner| {
        let mut found: Vec<(&JavaSummary, &Declaration, &Parameter, Slot)> = types
            .declarations_of(owner)
            .filter(|(_, declaration)| {
                declaration.is_constructor == argument.is_constructor
                    && declaration.name == argument.name
                    && accepts(declaration, argument.arity)
            })
            .filter_map(|(declaring, declaration)| {
                let (parameter, slot) = parameter_at(declaration, argument)?;
                receives(parameter, argument).then_some((declaring, declaration, parameter, slot))
            })
            .collect();
        if found.iter().any(|(.., slot)| *slot == Slot::Whole) {
            found.retain(|(.., slot)| *slot == Slot::Whole);
        }
        (!found.is_empty()).then_some(found)
    })?;
    let mut names = candidates
        .iter()
        .map(|(declaring, declaration, parameter, slot)| {
            (*slot == Slot::Whole && inferred_non_null(declaration, parameter, argument)
                || marked_non_null(types, declaring, declaration, parameter))
            .then(|| parameter.name.clone())
        });
    let first = names.next()??;
    names.all(|other| other.is_some()).then_some(first)
}

// Lo que IntelliJ infiere sin anotaciones: el método lo desreferencia antes que nada y nadie lo
// puede redefinir (o se sabe cuál se llama).
fn inferred_non_null(declaration: &Declaration, parameter: &Parameter, argument: &NullArgument) -> bool {
    (declaration.inference != NullInference::Overridable || argument.exact)
        && declaration.body == Body::Written
        && parameter.dereferenced == Dereferenced::AlwaysFirst
}

// Bajo `@NullMarked`, un parámetro sin anotar es no nulo salvo que su tipo sea una variable que
// admite null, aunque el método se pueda redefinir o no tenga cuerpo.
fn marked_non_null(
    types: &ProjectTypes<'_>,
    declaring: &JavaSummary,
    declaration: &Declaration,
    parameter: &Parameter,
) -> bool {
    types.is_null_marked(declaring, &declaration.owner)
        && parameter.nullness == ParameterNullness::Unannotated
}

// Si la sobrecarga puede recibir este `null`: un primitivo no, y con un casteo sólo la del tipo
// casteado o una cuyo tipo es una variable de tipo.
fn receives(parameter: &Parameter, argument: &NullArgument) -> bool {
    parameter.accepts_null
        && argument
            .cast
            .as_deref()
            .is_none_or(|cast| parameter.type_variable || simple_type_name(cast) == parameter.type_name)
}

fn library_non_null(library: &Library, class: &str, argument: &NullArgument) -> Option<String> {
    let supertypes = library.supertypes(class);
    LIBRARY_NON_NULL
        .iter()
        .find(|(owner, name, arity, index, _)| {
            supertypes.iter().any(|supertype| supertype == owner)
                && *name == argument.name
                && *arity == argument.arity
                && *index == argument.index
        })
        .map(|(.., parameter)| (*parameter).to_owned())
}

// Qué recibe el parámetro con el `null`: el valor entero, o uno de los elementos del varargs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Whole,
    Element,
}

// Un `null` en el lugar del varargs es el arreglo entero sólo si es el único argumento de ahí;
// si no, es uno de sus elementos.
fn parameter_at<'declaration>(
    declaration: &'declaration Declaration,
    argument: &NullArgument,
) -> Option<(&'declaration Parameter, Slot)> {
    let last = declaration.parameters.len().checked_sub(1)?;
    if declaration.arity == Arity::Variadic && argument.index >= last {
        let cast_to_element = argument
            .cast
            .as_deref()
            .is_some_and(|cast| !cast.ends_with(ARRAY_SUFFIX));
        let slot = if argument.arity == declaration.parameters.len() && !cast_to_element {
            Slot::Whole
        } else {
            Slot::Element
        };
        return declaration
            .parameters
            .get(last)
            .map(|parameter| (parameter, slot));
    }
    declaration
        .parameters
        .get(argument.index)
        .map(|parameter| (parameter, Slot::Whole))
}

pub(super) const fn accepts(declaration: &Declaration, arity: usize) -> bool {
    let parameters = declaration.parameters.len();
    match declaration.arity {
        Arity::Fixed => arity == parameters,
        Arity::Variadic => arity.saturating_add(1) >= parameters,
    }
}
