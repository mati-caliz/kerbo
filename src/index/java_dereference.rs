use super::java::{Declaration, JavaSummary, NullInference, Position, lint_at};
use super::java_jdk::{is_always_defined, jdk_method};
use super::java_null_arguments::{accepts, non_null_parameter, reaches};
use super::java_nullness::{ProjectTypes, Resolved, library_ancestors, project_lineage, resolve};
use crate::jvm::library::Library;
use crate::lint::LintFinding;
use crate::lint::java_calls::{Arity, Body, ReturnType};
use crate::lint::java_chains::{Chain, ChainRoot, ChainStep};
use crate::lint::java_nullable_fields::ReturnNullness;
use crate::lint::java_owner::OwnerKind;
use crate::rules::JavaRule;

const QUALIFIER_SEPARATOR: char = '.';
const NULLABLE: &str = "Nullable";

// Calca el DataFlowIssue de IntelliJ en sus dos formas sin flujo entre métodos: en una cadena de
// llamadas, un paso que la librería declara `@Nullable` seguido de otra llamada sobre su resultado
// (`respuesta.getBody().error()`), y una variable asignada desde una llamada `@Nullable` usada sin
// chequeo en algún camino (`Cache cache = manager.getCache(nombre); cache.clear();`).
pub fn nullable_dereferences(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = summary
        .chains
        .iter()
        .flat_map(|chain| {
            let nullable = nullable_steps(types, library, summary, chain);
            let unboxing = chain
                .unboxed
                .zip(chain.steps.last())
                .filter(|(_, last)| {
                    nullable.len() == chain.steps.len()
                        && nullable.last() == Some(&true)
                        && !known_not_null(types, library, summary, last)
                })
                .map(|((line, column), last)| report(line, column, &last.name));
            chain
                .steps
                .windows(2)
                .zip(nullable.iter().copied())
                .filter_map(|(pair, is_nullable)| match pair {
                    [step, next] if is_nullable && !known_not_null(types, library, summary, step) => {
                        Some(report(next.line, next.column, &next.name))
                    }
                    _ => None,
                })
                .chain(unboxing)
                .collect::<Vec<_>>()
        })
        .collect();
    for flow in &summary.null_flows {
        if reaches(types, library, summary, flow) {
            findings.extend(
                flow.dereferences
                    .iter()
                    .map(|used| report(used.line, used.column, &used.name)),
            );
        }
    }
    findings
}

// Si el último paso de la cadena, resuelto entero, puede devolver null.
pub(super) fn may_return_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> bool {
    let nullable = nullable_steps(types, library, summary, chain);
    nullable.len() == chain.steps.len() && nullable.last() == Some(&true)
}

// Si antes se chequeó contra null o se pasó a un parámetro que no puede ser null.
pub(super) fn known_not_null(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    step: &ChainStep,
) -> bool {
    step.checked
        || step
            .passed_before
            .iter()
            .any(|argument| non_null_parameter(types, library, summary, argument).is_some())
}

fn report(line: usize, column: usize, name: &str) -> LintFinding {
    lint_at(
        JavaRule::DataFlowIssue,
        &Position { line, column },
        format!("{name}()"),
    )
}

// Por cada paso de la cadena que se pudo resolver en la librería, si devuelve `@Nullable`. El tipo
// de cada paso sale de la firma del anterior; con sobrecargas del mismo nombre y aridad, sólo si
// todas dicen lo mismo. Lo del JDK que IntelliJ da por posiblemente null va primero: `ct.sym` no
// trae anotaciones.
fn nullable_steps(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    chain: &Chain,
) -> Vec<bool> {
    let mut current = resolve_root(types, library, summary, &chain.root);
    let mut nullable = Vec::new();
    for step in &chain.steps {
        if let Resolved::Project(owner) = &current
            && let Some((is_nullable, returned)) = project_step(types, library, owner, step)
        {
            nullable.push(is_nullable);
            let Some(returned) = returned else {
                break;
            };
            current = returned;
            continue;
        }
        let classes = match &current {
            Resolved::Library(class) => vec![class.clone()],
            Resolved::Project(name) => library_ancestors(types, library, name),
        };
        if let Some((returned, is_nullable)) = jdk_method(&classes, &step.name, step.arity) {
            nullable.push(is_nullable && !is_always_defined(&classes, &step.name, &step.text_arguments));
            current = Resolved::Library(returned.to_owned());
            continue;
        }
        let overloads = classes
            .iter()
            .map(|class| library.overloads(class, &step.name, step.arity))
            .find(|found| !found.is_empty())
            .unwrap_or_default();
        let by_class_arguments: Vec<_> = overloads
            .iter()
            .filter(|method| {
                step.class_arguments
                    .iter()
                    .all(|position| method.takes_class_at(*position))
            })
            .cloned()
            .collect();
        let candidates = if by_class_arguments.is_empty() {
            overloads
        } else {
            by_class_arguments
        };
        let Some(first) = candidates.first() else {
            break;
        };
        nullable.push(candidates.iter().all(|method| {
            method
                .return_annotations
                .iter()
                .any(|annotation| annotation.rsplit(QUALIFIER_SEPARATOR).next() == Some(NULLABLE))
        }));
        let returned = first.return_class();
        if candidates.iter().any(|method| method.return_class() != returned) {
            break;
        }
        let Some(returned) = returned else {
            break;
        };
        current = Resolved::Library(returned);
    }
    nullable
}

// El tipo de la raíz: el de la variable, la clase, o el de una constante importada con
// `import static` (`OS_NAME.value()`), que es un valor de su enum o un campo estático.
fn resolve_root(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    root: &ChainRoot,
) -> Resolved {
    match root {
        ChainRoot::Instance(written) => resolve(types, library, summary, written),
        ChainRoot::Class(written) => imported_constant(types, library, summary, written)
            .unwrap_or_else(|| resolve(types, library, summary, written)),
    }
}

fn imported_constant(
    types: &ProjectTypes<'_>,
    library: &Library,
    summary: &JavaSummary,
    name: &str,
) -> Option<Resolved> {
    if types.is_declared(name) {
        return None;
    }
    let imported_from = summary.static_imports.iter().find_map(|path| {
        let (owner, member) = path.rsplit_once(QUALIFIER_SEPARATOR)?;
        (member == name).then_some(owner)
    })?;
    let Resolved::Project(owner) = resolve(types, library, summary, imported_from) else {
        return None;
    };
    if let Some((declaring, field)) = types.field_of(&owner, name) {
        return field
            .is_static
            .then(|| resolve(types, library, declaring, &field.type_name));
    }
    let is_enum = types
        .declarations_of(&owner)
        .any(|(_, declaration)| declaration.owner.kind == OwnerKind::Enum);
    is_enum.then_some(Resolved::Project(owner))
}

// Un paso que va a un método del proyecto (de la clase o de las de arriba en el proyecto): es
// `@Nullable` si lo declara así, o si IntelliJ lo infiere: devuelve `null` en algún `return` y
// nadie puede redefinirlo. Sigue por
// el tipo que declara, si todas las sobrecargas dicen el mismo. Como Java, una sobrecarga de aridad
// fija gana a una varargs, y la que declara justo el tipo conocido de cada argumento gana al resto.
fn project_step(
    types: &ProjectTypes<'_>,
    library: &Library,
    owner: &str,
    step: &ChainStep,
) -> Option<(bool, Option<Resolved>)> {
    let candidates: Vec<_> = project_lineage(types, library, owner).iter().find_map(|class| {
        let found: Vec<_> = types
            .declarations_of(class)
            .filter(|(_, declaration)| {
                !declaration.is_constructor
                    && declaration.name == step.name
                    && accepts(declaration, step.arity)
            })
            .collect();
        let fixed: Vec<_> = found
            .iter()
            .copied()
            .filter(|(_, declaration)| declaration.arity == Arity::Fixed)
            .collect();
        let preferred = if fixed.is_empty() { found } else { fixed };
        let exact: Vec<_> = preferred
            .iter()
            .copied()
            .filter(|(_, declaration)| declares_argument_types(declaration, &step.argument_types))
            .collect();
        let chosen = if exact.is_empty() { preferred } else { exact };
        (!chosen.is_empty()).then_some(chosen)
    })?;
    let is_nullable = candidates.iter().all(|(_, declaration)| {
        declaration.return_nullness == ReturnNullness::Nullable
            || (declaration.inference == NullInference::ClosedReturnsNull
                && declaration.body == Body::Written)
    });
    let mut returned = candidates.iter().map(|(declaring, declaration)| {
        (declaration.return_type == ReturnType::Reference)
            .then(|| resolve(types, library, declaring, &declaration.returned))
    });
    let first = returned.next().flatten();
    let same = returned.all(|other| other == first);
    Some((is_nullable, first.filter(|_| same)))
}

fn declares_argument_types(declaration: &Declaration, argument_types: &[Option<String>]) -> bool {
    argument_types.iter().enumerate().all(|(index, known)| {
        let parameter = declaration
            .parameters
            .get(index)
            .or_else(|| declaration.parameters.last());
        known
            .as_ref()
            .zip(parameter)
            .is_none_or(|(argument, parameter)| parameter.type_name == *argument)
    })
}
