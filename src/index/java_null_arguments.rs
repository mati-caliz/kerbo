use super::java::{
    Declaration, Dereferenced, JavaSummary, NullInference, Parameter, ParameterNullness, Position, lint_at,
};
use super::java_dereference::{known_not_null, may_return_null};
use super::java_inferred_arguments::context_inferred_non_null;
use super::java_library_arguments::{called_type, inherited_library_class, library_substituted_non_null};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body};
use crate::lint::java_fields::Volatility;
use crate::lint::java_null_flow::{NullFlow, NullOrigin};
use crate::lint::java_null_inference::{ARRAY_SUFFIX, NullArgument};
use crate::lint::java_receiver_arguments::ArgumentNullness;
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
    ("java.util.Map", "merge", 3, 1, "value"),
    ("java.util.Map", "merge", 3, 2, "remappingFunction"),
    ("java.util.List", "addAll", 2, 1, "c"),
    ("java.io.File", "File", 1, 0, "pathname"),
    ("java.nio.file.Paths", "get", 1, 0, "first"),
    ("java.util.concurrent.ConcurrentMap", "remove", 2, 0, "key"),
];

// Calca el "Passing 'null' argument to parameter annotated as non-null" de DataFlowIssue con lo que
// IntelliJ infiere sin anotaciones: un parámetro que el método desreferencia en todos los caminos es
// no nulo, si nadie puede redefinir el método. Con sobrecargas, todas tienen que coincidir. También
// el "Argument might be null": una llamada que puede devolver null, o una variable asignada desde
// una, pasada así a ese parámetro.
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
    let nullable_calls = summary.chains.iter().filter_map(|chain| {
        let argument = chain.passed_as.as_deref()?;
        let last = chain.steps.last()?;
        (may_return_null(types, library, summary, chain) && !known_not_null(types, library, summary, last))
            .then_some(argument)
    });
    let inferred = summary.nested_nulls.iter().filter_map(|nested| {
        if non_null_parameter(types, library, summary, &nested.inner).is_some() {
            return None;
        }
        let parameter = context_inferred_non_null(types, library, summary, nested)?;
        Some(report(&nested.inner, parameter))
    });
    summary
        .null_arguments
        .iter()
        .chain(possibly_null)
        .chain(nullable_calls)
        .filter_map(|argument| {
            let parameter = non_null_parameter(types, library, summary, argument)?;
            Some(report(argument, parameter))
        })
        .chain(inferred)
        .collect()
}

const fn report(argument: &NullArgument, parameter: String) -> LintFinding {
    lint_at(
        JavaRule::NullArgument,
        &Position {
            line: argument.line,
            column: argument.column,
        },
        parameter,
    )
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
        NullOrigin::NullableField | NullOrigin::Literal => true,
        NullOrigin::QualifiedField {
            owner,
            via,
            field,
            after_call,
        } => is_nullable_field(
            types,
            library,
            summary,
            (owner, via.as_deref(), field, *after_call),
        ),
    };
    origin_may_be_null
        && flow
            .passed_before
            .iter()
            .all(|argument| non_null_parameter(types, library, summary, argument).is_none())
}

// Si el campo que declara la clase del proyecto (o una de arriba en el proyecto) es `@Nullable` y,
// leído de nuevo después de una llamada, si es `volatile`. Con un campo arreglo en el medio, el
// campo es del tipo de sus elementos.
fn is_nullable_field(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    (owner, via, field, after_call): (&str, Option<&str>, &str, bool),
) -> bool {
    let Resolved::Project(mut owner) = resolve(types, library, summary, owner) else {
        return false;
    };
    if let Some(via) = via {
        let Some((declaring, array)) = project_lineage(types, library, &owner)
            .iter()
            .find_map(|class| types.field_of(class, via))
        else {
            return false;
        };
        let Resolved::Project(element) = resolve(types, library, declaring, &array.type_name) else {
            return false;
        };
        owner = element;
    }
    project_lineage(types, library, &owner)
        .iter()
        .find_map(|class| types.field_of(class, field))
        .is_some_and(|(_, declared)| {
            declared.is_nullable && (!after_call || declared.volatility == Volatility::Volatile)
        })
}

// Las clases del proyecto donde buscar el método que recibe el argumento, de la más cercana a la
// más lejana. Vacío si el receptor es de una librería o no se sabe.
pub(super) fn project_owners(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Vec<String> {
    match &argument.receiver {
        Receiver::Type(written) => match resolve(types, library, summary, written) {
            Resolved::Project(owner) if argument.is_constructor => vec![owner],
            Resolved::Project(owner) => project_lineage(types, library, &owner),
            Resolved::Library(_) => Vec::new(),
        },
        Receiver::Scope(enclosing) => enclosing
            .iter()
            .flat_map(|owner| project_lineage(types, library, owner))
            .collect(),
        Receiver::Unknown => Vec::new(),
    }
}

pub(super) fn non_null_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Option<String> {
    if let Receiver::Type(written) = &argument.receiver
        && let Resolved::Library(class) = resolve(types, library, summary, written)
    {
        return library_non_null(library, &class, argument).or_else(|| {
            library_substituted_non_null(
                library,
                &class,
                argument,
                types.is_marked(summary, argument.receiver_arguments.marking),
            )
        });
    }
    if argument.receiver == Receiver::Unknown
        && let Some(called) = &argument.receiver_arguments.called
        && let Some(returned) = called_type(types, library, summary, called)
    {
        let mut through_returned = argument.clone();
        through_returned.receiver = Receiver::Type(returned);
        through_returned.receiver_arguments.called = None;
        return non_null_parameter(types, library, summary, &through_returned);
    }
    if argument.receiver == Receiver::Unknown
        && let Some(member) = &argument.receiver_arguments.inherited
        && let Some(class) = inherited_library_class(types, library, member)
    {
        return library_non_null(library, &class, argument);
    }
    let owners = project_owners(types, library, summary, argument);
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
                || marked_non_null(types, declaring, declaration, parameter)
                || substituted_non_null(types, summary, (*declaring, *declaration, *parameter), argument))
            .then(|| parameter.name.clone())
        });
    let first = names.next()??;
    names.all(|other| other.is_some()).then_some(first)
}

// Lo que IntelliJ infiere sin anotaciones: el método lo desreferencia antes que nada y nadie lo
// puede redefinir (o se sabe cuál se llama), y no lo declara `@Nullable`.
fn inferred_non_null(declaration: &Declaration, parameter: &Parameter, argument: &NullArgument) -> bool {
    (declaration.inference != NullInference::Overridable || argument.exact)
        && declaration.body == Body::Written
        && parameter.dereferenced == Dereferenced::AlwaysFirst
        && parameter.nullness != ParameterNullness::Nullable
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

// Bajo `@NullMarked` de los dos lados, un parámetro cuyo tipo es una variable de tipo de la clase
// que admite null deja de admitirlo si el receptor se declara con un tipo concreto en su lugar
// (`Multiset<String> ms` en `ms.setCount(null, 1, 0)`).
fn substituted_non_null(
    types: &ProjectTypes<'_>,
    caller: &JavaSummary,
    (declaring, declaration, parameter): (&JavaSummary, &Declaration, &Parameter),
    argument: &NullArgument,
) -> bool {
    let receiver = &argument.receiver_arguments;
    let declared_on_receiver = match &argument.receiver {
        Receiver::Type(written) => simple_type_name(written) == declaration.owner.name,
        Receiver::Scope(_) | Receiver::Unknown => false,
    };
    if parameter.nullness != ParameterNullness::NullableVariable
        || !declared_on_receiver
        || !types.is_null_marked(declaring, &declaration.owner)
        || !types.is_marked(caller, receiver.marking)
        || declares_method_variable(declaring, declaration, &parameter.type_name)
    {
        return false;
    }
    declaring
        .generics
        .types
        .iter()
        .find(|generics| generics.name == declaration.owner.name)
        .and_then(|generics| {
            generics
                .parameters
                .iter()
                .position(|name| *name == parameter.type_name)
        })
        .is_some_and(|position| receiver.arguments.get(position) == Some(&ArgumentNullness::Concrete))
}

pub(super) fn declares_method_variable(
    declaring: &JavaSummary,
    declaration: &Declaration,
    variable: &str,
) -> bool {
    declaring.generics.methods.iter().any(|method| {
        method.line == declaration.position.line
            && method.column == declaration.position.column
            && method.parameters.iter().any(|name| name == variable)
    })
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
pub(super) enum Slot {
    Whole,
    Element,
}

// Un `null` en el lugar del varargs es el arreglo entero sólo si es el único argumento de ahí;
// si no, es uno de sus elementos.
pub(super) fn parameter_at<'declaration>(
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
