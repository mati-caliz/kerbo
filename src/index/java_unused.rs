use super::java::{Declaration, lint_at};
use super::java_project::{IndexedSummary, Project};
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

// Si el proyecto es una aplicación: en una librería, lo público es su API y no se espera que se use adentro.
pub struct Usage {
    is_application: bool,
}

impl Usage {
    // En una librería, lo público y lo protegido es API: quien lo usa (o lo redefine) está afuera.
    pub const fn sees_callers_of(&self, visibility: Visibility) -> bool {
        self.is_application || matches!(visibility, Visibility::Private | Visibility::Package)
    }

    pub fn new(files: &[IndexedSummary<'_>]) -> Self {
        let is_application = files.iter().any(|file| {
            file.summary.declarations.iter().any(|declaration| {
                declaration
                    .owner
                    .annotations
                    .iter()
                    .any(|annotation| APPLICATION_ANNOTATIONS.contains(&annotation.as_str()))
            })
        });
        Self { is_application }
    }
}

// Un método que nada del proyecto nombra y que ningún framework conocido llama. Lo público y lo
// protegido sólo en una aplicación.
pub fn unused_method(
    project: &Project<'_>,
    declaration: &Declaration,
    file: &IndexedSummary<'_>,
    usage: &Usage,
) -> Option<LintFinding> {
    let unused = !declaration.is_constructor
        && !declaration.overrides
        && !project.is_named(declaration)
        && usage.sees_callers_of(declaration.visibility)
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
