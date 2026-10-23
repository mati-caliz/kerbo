use super::java::{Declaration, Field, Reading, lint_at};
use super::java_project::{IndexedSummary, Project};
use crate::lint::LintFinding;
use crate::lint::java_calls::{Body, Visibility};
use crate::lint::java_owner::OwnerKind;
use crate::rules::JavaRule;

const MAIN: &str = "main";
// Los métodos de serialización que llama la JVM con una firma fija.
const SERIALIZATION: &[&str] = &["readObject", "writeObject", "readObjectNoData"];
// Los parámetros de un test parametrizado los impone la fuente de argumentos.
const PARAMETERIZED: &[&str] = &["ParameterizedTest", "Theory"];
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
// La inyección escribe el campo, pero si nadie lo lee sobra igual. `@Mock`, `@Spy` y `@MockitoBean`
// no están: se inyectan en lo que se prueba o reemplazan un bean del contexto.
const INJECTION_ANNOTATIONS: &[&str] = &[
    "Autowired",
    "Inject",
    "Resource",
    "Value",
    "Qualifier",
    "PersistenceContext",
    "LocalServerPort",
];
// Anotaciones de clase que no leen los campos de instancia: los estereotipos de Spring, la
// configuración de los tests y los constructores de Lombok. Con cualquier otra (`@Data`, `@Getter`,
// `@Entity`, `@JsonIgnoreProperties`, `@ConfigurationProperties`, una propia), un framework puede
// estar leyéndolos.
const FIELD_BLIND_OWNERS: &[&str] = &[
    "Service",
    "Component",
    "Controller",
    "RestController",
    "Repository",
    "Configuration",
    "TestConfiguration",
    "RequestMapping",
    "Validated",
    "Order",
    "Profile",
    "ExtendWith",
    "SpringBootTest",
    "WebMvcTest",
    "DataJpaTest",
    "JdbcTest",
    "JsonTest",
    "RestClientTest",
    "AutoConfigureMockMvc",
    "AutoConfigureTestDatabase",
    "ActiveProfiles",
    "Import",
    "TestPropertySource",
    "DirtiesContext",
    "Sql",
    "Testcontainers",
    "DisplayName",
    "Tag",
    "Nested",
    "TestInstance",
    "Execution",
    "RequiredArgsConstructor",
    "NoArgsConstructor",
    "AllArgsConstructor",
    "Slf4j",
    "Log4j2",
];
// Los campos que lee la serialización de Java por su nombre.
const SERIALIZATION_FIELDS: &[&str] = &["serialVersionUID", "serialPersistentFields"];
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
            .all(|annotation| is_passive(annotation))
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

// Un campo que nada del proyecto nombra, ni en el código ni en un texto. Uno con una anotación que
// no es pasiva ni de inyección (`@Column`, `@Mock`, `@JsonProperty`...) lo usa un framework, y los
// de instancia de una clase con una anotación que puede leerlos (Lombok, JPA, Jackson) también. Lo
// privado se mira en su archivo; lo público y protegido, sólo en una aplicación.
pub fn unused_field(
    project: &Project<'_>,
    field: &Field,
    file: &IndexedSummary<'_>,
    usage: &Usage,
) -> Option<LintFinding> {
    let unused = framework_ignores(field)
        && if field.visibility == Visibility::Private || field.owner.kind == OwnerKind::Local {
            !file_mentions(file, &field.name)
        } else {
            usage.sees_callers_of(field.visibility) && !project.mentions_identifier(&field.name)
        };
    unused.then(|| lint_at(JavaRule::UnusedField, &field.position, field.name.clone()))
}

// Si ningún framework conocido lee el campo: ni la serialización, ni por una anotación propia, ni
// por una de su clase.
fn framework_ignores(field: &Field) -> bool {
    !SERIALIZATION_FIELDS.contains(&field.name.as_str())
        && field
            .annotations
            .iter()
            .all(|annotation| is_passive(annotation) || INJECTION_ANNOTATIONS.contains(&annotation.as_str()))
        && (field.is_static
            || field.owner.annotations.iter().all(|annotation| {
                is_passive(annotation) || FIELD_BLIND_OWNERS.contains(&annotation.as_str())
            }))
}

// Un campo privado que el código de su archivo sólo escribe: el valor no se usa nunca. Las mismas
// tablas que `unused_field` deciden si lo lee un framework.
// El "Parameter is not used" de IntelliJ: un parámetro que el cuerpo nunca nombra, en un método que no
// redefine a otro ni es redefinido (ahí la firma la impone la jerarquía), que no es un `main`, un
// método de serialización ni un test parametrizado. Sin `@Override`, un método puede implementar uno
// de una clase emparentada del proyecto o de una librería: si hay uno igual en la jerarquía, o si la
// clase hereda algo de afuera y el método no es privado, no se marca.
pub fn unused_parameters(project: &Project<'_>, declaration: &Declaration) -> Vec<LintFinding> {
    let entry_point = declaration.name == MAIN && declaration.parameters.len() == 1;
    if declaration.body != Body::Written
        || declaration.overrides
        || entry_point
        || declaration
            .annotations
            .iter()
            .any(|annotation| PARAMETERIZED.contains(&annotation.as_str()))
        || project.is_overridden(declaration)
        || project.has_twin(declaration)
        || (declaration.visibility == Visibility::Private
            && SERIALIZATION.contains(&declaration.name.as_str()))
        || (declaration.visibility != Visibility::Private
            && project.has_foreign_ancestor(&declaration.owner.name))
    {
        return Vec::new();
    }
    declaration
        .parameters
        .iter()
        .filter(|parameter| parameter.reading == Reading::Unread)
        .map(|parameter| {
            lint_at(
                JavaRule::UnusedParameter,
                &parameter.position,
                parameter.name.clone(),
            )
        })
        .collect()
}

pub fn unread_field(field: &Field, file: &IndexedSummary<'_>) -> Option<LintFinding> {
    let summary = file.summary;
    let unread = (field.visibility == Visibility::Private || field.owner.kind == OwnerKind::Local)
        && framework_ignores(field)
        && summary
            .written_only
            .binary_search_by(|name| name.as_str().cmp(&field.name))
            .is_ok()
        && !summary.mentioned.contains(&field.name);
    unread.then(|| lint_at(JavaRule::UnreadField, &field.position, field.name.clone()))
}

fn is_passive(annotation: &str) -> bool {
    PASSIVE_ANNOTATIONS.contains(&annotation)
}

fn file_mentions(file: &IndexedSummary<'_>, name: &str) -> bool {
    let summary = file.summary;
    summary
        .identifiers
        .binary_search_by(|identifier| identifier.as_str().cmp(name))
        .is_ok()
        || summary.mentioned.iter().any(|mentioned| mentioned == name)
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
