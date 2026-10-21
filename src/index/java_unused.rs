use std::collections::HashSet;

use super::java::{Declaration, lint_at};
use super::java_rules::IndexedSummary;
use crate::lint::LintFinding;
use crate::lint::java_calls::Visibility;
use crate::lint::java_owner::OwnerKind;
use crate::rules::JavaRule;

// Anotaciones que no dicen que un framework llame al método: con cualquier otra (`@GetMapping`,
// `@Bean`, `@PrePersist`, `@Test`, `@JsonCreator`, `@Scheduled`, una propia...), se lo da por usado.
const PASSIVE_ANNOTATIONS: &[&str] = &[
    "Deprecated",
    "SuppressWarnings",
    "SafeVarargs",
    "Nullable",
    "NonNull",
    "Nonnull",
    "NotNull",
    "CheckForNull",
    "Transactional",
    "Async",
    "Retryable",
    "Cacheable",
    "CacheEvict",
    "CachePut",
    "PreAuthorize",
    "PostAuthorize",
    "Secured",
    "RolesAllowed",
    "Timed",
    "Counted",
    "VisibleForTesting",
    "CanIgnoreReturnValue",
    "CheckReturnValue",
    "Contract",
    "Pure",
    "Beta",
    "GwtIncompatible",
    "J2ktIncompatible",
    "Valid",
];
// Tipos cuyos métodos llama el framework y no el código: los de un mapper de MapStruct, un
// aspecto, un endpoint de Actuator o un recurso de JAX-RS.
const FRAMEWORK_OWNERS: &[&str] = &[
    "Mapper",
    "Aspect",
    "Endpoint",
    "WebEndpoint",
    "RestControllerEndpoint",
    "ControllerEndpoint",
    "Path",
];
// Métodos que llama el JDK o JUnit 3 por su nombre, aunque no digan `@Override`.
const CALLED_BY_NAME: &[&str] = &[
    "main",
    "toString",
    "equals",
    "hashCode",
    "clone",
    "finalize",
    "compareTo",
    "compare",
    "writeObject",
    "readObject",
    "readObjectNoData",
    "readResolve",
    "writeReplace",
    "close",
    "run",
    "call",
    "iterator",
    "apply",
    "accept",
    "test",
    "get",
    "hasNext",
    "next",
    "remove",
    "uncaughtException",
    "setUp",
    "tearDown",
    "suite",
];
const ACCESSOR_PREFIXES: &[&str] = &["get", "is", "set"];
const JUNIT3_TEST_PREFIX: &str = "test";
// Una aplicación se reconoce por la anotación de su framework: un `main` solo no alcanza, porque
// las librerías también traen runners de consola.
const APPLICATION_ANNOTATIONS: &[&str] = &["SpringBootApplication", "QuarkusMain", "MicronautApplication"];

// Qué nombres usa el proyecto (llamadas, referencias a métodos y textos que nombran un método) y
// si es una aplicación: en una librería, lo público es su API y no se espera que se use adentro.
pub struct Usage<'summary> {
    names: HashSet<&'summary str>,
    is_application: bool,
}

impl<'summary> Usage<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut names = HashSet::new();
        let mut is_application = false;
        for file in files {
            let summary = file.summary;
            names.extend(
                summary
                    .calls
                    .iter()
                    .filter(|call| !call.is_constructor)
                    .map(|call| call.name.as_str()),
            );
            names.extend(summary.referenced.iter().map(String::as_str));
            names.extend(summary.mentioned.iter().map(String::as_str));
            is_application = is_application
                || summary.declarations.iter().any(|declaration| {
                    declaration
                        .owner
                        .annotations
                        .iter()
                        .any(|annotation| APPLICATION_ANNOTATIONS.contains(&annotation.as_str()))
                });
        }
        Self {
            names,
            is_application,
        }
    }
}

// Un método que nada del proyecto nombra y que ningún framework conocido llama. Lo público y lo
// protegido sólo en una aplicación.
pub fn unused_method(
    declaration: &Declaration,
    file: &IndexedSummary<'_>,
    usage: &Usage<'_>,
) -> Option<LintFinding> {
    let exposed = matches!(declaration.visibility, Visibility::Public | Visibility::Protected);
    let unused = !declaration.is_constructor
        && !declaration.overrides
        && !usage.names.contains(declaration.name.as_str())
        && (usage.is_application || !exposed)
        && !matches!(declaration.owner.kind, OwnerKind::Local | OwnerKind::Annotation)
        && declaration
            .annotations
            .iter()
            .all(|annotation| PASSIVE_ANNOTATIONS.contains(&annotation.as_str()))
        && !declaration
            .owner
            .annotations
            .iter()
            .any(|annotation| FRAMEWORK_OWNERS.contains(&annotation.as_str()))
        && !is_called_by_convention(&declaration.name, file);
    unused.then(|| {
        lint_at(
            JavaRule::UnusedMethod,
            &declaration.position,
            declaration.name.clone(),
        )
    })
}

// Los accessors los usan Jackson, JPA y las plantillas; algunos nombres los llama el JDK; en los
// tests, `testAlgo()` es un test de JUnit 3.
fn is_called_by_convention(name: &str, file: &IndexedSummary<'_>) -> bool {
    let accessor = ACCESSOR_PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|first| !first.is_ascii_lowercase())
    });
    accessor || CALLED_BY_NAME.contains(&name) || (file.in_tests && name.starts_with(JUNIT3_TEST_PREFIX))
}
