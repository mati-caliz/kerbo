use super::java::{Declaration, Dereferenced, JavaSummary, NullInference, Parameter, Position, lint_at};
use super::java_nullness::{ProjectTypes, Resolved, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body};
use crate::lint::java_null_inference::NullArgument;
use crate::lint::java_receivers::Receiver;
use crate::rules::JavaRule;

// Calca el "Passing 'null' argument to parameter annotated as non-null" de DataFlowIssue con lo que
// IntelliJ infiere sin anotaciones: un parámetro que el método desreferencia en todos los caminos es
// no nulo, si nadie puede redefinir el método. Con sobrecargas, todas tienen que coincidir.
pub fn null_arguments(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    summary
        .null_arguments
        .iter()
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

fn non_null_parameter(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    argument: &NullArgument,
) -> Option<String> {
    let owners: Vec<String> = match &argument.receiver {
        Receiver::Type(written) => match resolve(types, library, summary, written) {
            Resolved::Project(owner) if argument.is_constructor => vec![owner],
            Resolved::Project(owner) => project_lineage(types, library, &owner),
            Resolved::Library(_) => return None,
        },
        Receiver::Scope(enclosing) => enclosing
            .iter()
            .flat_map(|owner| project_lineage(types, library, owner))
            .collect(),
        Receiver::Unknown => return None,
    };
    let candidates = owners.iter().find_map(|owner| {
        let found: Vec<&Declaration> = types
            .declarations_of(owner)
            .map(|(_, declaration)| declaration)
            .filter(|declaration| {
                declaration.is_constructor == argument.is_constructor
                    && declaration.name == argument.name
                    && accepts(declaration, argument.arity)
            })
            .collect();
        (!found.is_empty()).then_some(found)
    })?;
    let mut names = candidates.iter().map(|declaration| {
        let parameter = parameter_at(declaration, argument)?;
        (declaration.inference != NullInference::Overridable
            && declaration.body == Body::Written
            && parameter.dereferenced == Dereferenced::AlwaysFirst)
            .then(|| parameter.name.clone())
    });
    let first = names.next()??;
    names.all(|other| other.is_some()).then_some(first)
}

// Un `null` en el lugar del varargs es el arreglo entero sólo si es el único argumento de ahí.
fn parameter_at<'declaration>(
    declaration: &'declaration Declaration,
    argument: &NullArgument,
) -> Option<&'declaration Parameter> {
    let last = declaration.parameters.len().checked_sub(1)?;
    if declaration.arity == Arity::Variadic && argument.index >= last {
        return (argument.arity == declaration.parameters.len())
            .then(|| declaration.parameters.get(last))
            .flatten();
    }
    declaration.parameters.get(argument.index)
}

const fn accepts(declaration: &Declaration, arity: usize) -> bool {
    let parameters = declaration.parameters.len();
    match declaration.arity {
        Arity::Fixed => arity == parameters,
        Arity::Variadic => arity.saturating_add(1) >= parameters,
    }
}
