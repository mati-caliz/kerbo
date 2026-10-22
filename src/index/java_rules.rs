use super::java::{CallSite, Declaration, JavaSummary, lint_at};
use super::java_dereference::nullable_dereferences;
use super::java_nullness::{ProjectTypes, unannotated_overrides};
use super::java_project::{IndexedSummary, Project, key_of};
use super::java_resources::unclosed_resources;
use super::java_taint::tainted_sinks;
use super::java_unused::{Usage, unused_field, unused_method};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body, Returns, Visibility};
use crate::rules::JavaRule;

const STRING_QUOTE: char = '"';

// Las inspecciones de IntelliJ que miran el proyecto entero, por archivo. Sólo valen si el
// análisis ve todo el proyecto: una llamada que falta cambia la respuesta.
pub fn project_lints(files: &[IndexedSummary<'_>], library: &Library) -> Vec<Vec<LintFinding>> {
    let project = Project::new(files);
    let types = ProjectTypes::new(files);
    let usage = Usage::new(files);
    let mut tainted = tainted_sinks(files, &project).into_iter();
    files
        .iter()
        .map(|file| {
            let summary = file.summary;
            let mut in_file = tainted.next().unwrap_or_default();
            in_file.extend(nullable_dereferences(&types, library, summary));
            in_file.extend(unclosed_resources(&types, library, summary));
            in_file.extend(
                summary
                    .fields
                    .iter()
                    .filter_map(|field| unused_field(&project, field, file, &usage)),
            );
            in_file.extend(summary.declarations.iter().flat_map(|declaration| {
                let mut found = Vec::new();
                if usage.sees_callers_of(declaration.visibility) {
                    found.extend(same_parameter_values(&project, file, declaration));
                    found.extend(unused_return_value(&project, declaration));
                }
                found.extend(same_return_value(&project, summary, declaration));
                found.extend(unused_method(&project, declaration, file, &usage));
                found.extend(unannotated_overrides(&types, library, summary, declaration));
                found
            }));
            in_file
        })
        .collect()
}

// SameParameterValue para lo de paquete y lo protegido (lo privado se mira por archivo, y IntelliJ
// no marca lo público).
fn same_parameter_values<'summary>(
    project: &Project<'summary>,
    file: &IndexedSummary<'_>,
    declaration: &'summary Declaration,
) -> Vec<LintFinding> {
    let visible_in_project = matches!(
        declaration.visibility,
        Visibility::Package | Visibility::Protected
    );
    if !visible_in_project
        || declaration.body == Body::Abstract
        || declaration.arity == Arity::Variadic
        || !project.is_traceable(declaration)
    {
        return Vec::new();
    }
    let calls = project.calls_to(declaration);
    declaration
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let value =
                shared_argument(&calls, index).filter(|value| project.is_movable_into(value, file))?;
            Some(lint_at(
                JavaRule::SameParameterValue,
                &parameter.position,
                format!("{} = {value}", parameter.name),
            ))
        })
        .collect()
}

fn shared_argument(calls: &[&CallSite], index: usize) -> Option<String> {
    let mut values = calls
        .iter()
        .map(|call| call.arguments.get(index).cloned().flatten());
    let first = values.next()??;
    values
        .all(|value| value.as_deref() == Some(first.as_str()))
        .then_some(first)
}

// UnusedReturnValue: un método que devuelve algo, al que todas las llamadas del proyecto le tiran
// el resultado.
fn unused_return_value<'summary>(
    project: &Project<'summary>,
    declaration: &'summary Declaration,
) -> Option<LintFinding> {
    if declaration.is_constructor
        || declaration.returns == Returns::Nothing
        || !project.is_traceable(declaration)
    {
        return None;
    }
    let calls = project.calls_to(declaration);
    (!calls.is_empty() && calls.iter().all(|call| !call.result_used)).then(|| {
        lint_at(
            JavaRule::UnusedReturnValue,
            &declaration.position,
            declaration.name.clone(),
        )
    })
}

// SameReturnValue: todos los `return` de un método devuelven la misma constante. Para lo privado
// alcanza con que no haya otro igual en su clase de nivel superior.
fn same_return_value<'summary>(
    project: &Project<'summary>,
    summary: &JavaSummary,
    declaration: &'summary Declaration,
) -> Option<LintFinding> {
    let Returns::Always(value) = &declaration.returns else {
        return None;
    };
    if declaration.body == Body::Abstract || declaration.overrides {
        return None;
    }
    let unique = if declaration.visibility == Visibility::Private {
        summary
            .declarations
            .iter()
            .filter(|other| other.scope == declaration.scope && key_of(other) == key_of(declaration))
            .count()
            == 1
    } else {
        !project.is_overridden(declaration)
    };
    unique.then(|| {
        lint_at(
            JavaRule::SameReturnValue,
            &declaration.position,
            format!("{}() = {}", declaration.name, shown(value)),
        )
    })
}

// IntelliJ muestra el texto de un literal de cadena sin las comillas.
fn shown(value: &str) -> &str {
    value
        .strip_prefix(STRING_QUOTE)
        .and_then(|inner| inner.strip_suffix(STRING_QUOTE))
        .unwrap_or(value)
}
