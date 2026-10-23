use super::java::{CallSite, Declaration, JavaSummary, lint_at};
use super::java_dereference::nullable_dereferences;
use super::java_escapes::{PackageTypes, escaped_types};
use super::java_nullness::{ProjectTypes, unannotated_overrides};
use super::java_project::{IndexedSummary, Project, key_of};
use super::java_resources::unclosed_resources;
use super::java_taint::tainted_sinks;
use super::java_unused::{Usage, unread_field, unused_field, unused_method, unused_parameters};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body, ResultUse, ReturnType, Returns, Visibility};
use crate::lint::java_receivers::Receiver;
use crate::rules::JavaRule;

const STRING_QUOTE: char = '"';
const MIN_INVERTED_CALLS: usize = 2;

// Las inspecciones de IntelliJ que miran el proyecto entero, por archivo. Sólo valen si el
// análisis ve todo el proyecto: una llamada que falta cambia la respuesta.
pub fn project_lints(files: &[IndexedSummary<'_>], library: &Library) -> Vec<Vec<LintFinding>> {
    let project = Project::new(files);
    let types = ProjectTypes::new(files);
    let usage = Usage::new(files);
    let package_types = PackageTypes::new(files);
    let mut tainted = tainted_sinks(files, &project).into_iter();
    files
        .iter()
        .map(|file| {
            let summary = file.summary;
            let mut in_file = tainted.next().unwrap_or_default();
            in_file.extend(nullable_dereferences(&types, library, summary));
            in_file.extend(unclosed_resources(&types, library, summary));
            in_file.extend(escaped_types(&package_types, summary));
            in_file.extend(
                summary
                    .fields
                    .iter()
                    .filter_map(|field| unused_field(&project, field, file, &usage)),
            );
            in_file.extend(
                summary
                    .fields
                    .iter()
                    .filter_map(|field| unread_field(field, file)),
            );
            in_file.extend(summary.declarations.iter().flat_map(|declaration| {
                let mut found = Vec::new();
                if usage.sees_callers_of(declaration.visibility) {
                    found.extend(same_parameter_values(&project, file, declaration));
                    found.extend(unused_return_value(&project, declaration));
                    found.extend(always_inverted(&project, declaration));
                }
                found.extend(same_return_value(&project, summary, declaration));
                if usage.sees_callers_of(declaration.visibility) {
                    found.extend(same_implemented_value(&project, declaration));
                }
                found.extend(unused_method(&project, declaration, file, &usage));
                found.extend(unused_parameters(&project, declaration));
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
    (!calls.is_empty() && calls.iter().all(|call| call.result == ResultUse::Discarded)).then(|| {
        lint_at(
            JavaRule::UnusedReturnValue,
            &declaration.position,
            declaration.name.clone(),
        )
    })
}

// BooleanMethodIsAlwaysInverted: todas las llamadas del proyecto lo niegan (`!activo()`), y son al
// menos dos: con una sola, invertirlo no gana nada. Un `!` sólo va delante de un boolean, así que
// alcanza con que devuelva un primitivo. Con homónimos en clases sin relación, sólo cuentan las
// llamadas que se sabe que van a ésta.
fn always_inverted<'summary>(
    project: &Project<'summary>,
    declaration: &'summary Declaration,
) -> Option<LintFinding> {
    if declaration.is_constructor
        || declaration.return_type != ReturnType::Primitive
        || declaration.body != Body::Written
        || !project.is_traceable(declaration)
        || project.is_overridden(declaration)
    {
        return None;
    }
    let strangers = project.has_strangers(declaration);
    let calls: Vec<&CallSite> = project
        .calls_to(declaration)
        .into_iter()
        .filter(|call| !strangers || !matches!(call.receiver, Receiver::Unknown))
        .collect();
    (calls.len() >= MIN_INVERTED_CALLS && calls.iter().all(|call| call.result == ResultUse::Negated)).then(
        || {
            lint_at(
                JavaRule::BooleanMethodIsAlwaysInverted,
                &declaration.position,
                declaration.name.clone(),
            )
        },
    )
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

// SameReturnValue para un método abstracto: todas sus implementaciones devuelven la misma constante.
// Hace falta al menos una en el código de producción: las proyecciones de Spring Data sólo tienen
// las de los tests, y su implementación real la arma Spring.
fn same_implemented_value(project: &Project<'_>, declaration: &Declaration) -> Option<LintFinding> {
    if declaration.body != Body::Abstract || project.may_be_functional(&declaration.owner.name) {
        return None;
    }
    let implementations = project.implementations(declaration);
    if implementations.iter().all(|(_, in_tests)| *in_tests) {
        return None;
    }
    let mut returned = implementations
        .iter()
        .map(|(implementation, _)| &implementation.returns);
    let Some(Returns::Always(value)) = returned.next() else {
        return None;
    };
    returned
        .all(|other| matches!(other, Returns::Always(same) if same == value))
        .then(|| {
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
