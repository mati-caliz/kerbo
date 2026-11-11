# Java

## Checkstyle

Las seis reglas de forma de Java imitan a Checkstyle 14 con `sun_checks.xml`, que es la
configuración de referencia que trae Checkstyle y la que ya se usa para `MagicNumber` y los
largos: `UnusedImports` (con `processJavadoc` apagado, como en `sun_checks`), `RedundantImport`,
`AvoidStarImport`, `RedundantModifier`, `FinalParameters` y `FinalClass`. `google_checks.xml` sólo
pide `AvoidStarImport`, y el quickstart de PMD 7 trae los equivalentes de imports, modificadores y
clases finales pero no `final` en los parámetros. `recommended` no pide `final` en los parámetros y
`minimal` sólo revisa imports sin usar y repetidos. Las columnas cuentan bytes, como en el resto de
las reglas: con tabs, Checkstyle las expande a 8 y da otro número.

Del resto de `sun_checks.xml`, kerbo hace los nombres (`TypeName`, `MethodName`, `ConstantName`,
`StaticVariableName`, `MemberName` y `ParameterName`, con los mismos nombres de regla en kebab:
`type-name`, `method-name`...), `UpperEll` (`upper-ell`, se arregla solo), `EmptyStatement`
(`empty-statement`), `ModifierOrder`, `ArrayTypeStyle`, `MultipleVariableDeclarations`,
`InterfaceIsType`, `MissingSwitchDefault`, `NeedBraces`, `EmptyBlock`, `AvoidNestedBlocks`,
`InnerAssignment`, `SimplifyBooleanExpression`, `EqualsHashCode`, `HideUtilityClassConstructor`,
`TodoComment`, `IllegalImport`, `FileTabCharacter`, los espacios al final de línea
(`trailing-spaces`, el `RegexpSingleline` de `sun_checks`), `NewlineAtEndOfFile`,
`DesignForExtension`, `HiddenField`, `VisibilityModifier`, `DeclarationOrder`,
`LocalFinalVariableName` y los de espacios y llaves: `WhitespaceAround`, `WhitespaceAfter`,
`NoWhitespaceBefore`, `NoWhitespaceAfter`, `GenericWhitespace`, `MethodParamPad`,
`EmptyForIteratorPad`, `LeftCurly`, `RightCurly` y `OperatorWrap`. Se arreglan solos los espacios
al final, `NewlineAtEndOfFile` y los de espacios (no las llaves ni `OperatorWrap`, que piden
reacomodar líneas). Siguen a Checkstyle en lo que no se ve: `MethodName` no mira lo que lleva
`@Override` y marca el método que se llama como su clase; un campo de interfaz es una constante;
`serialVersionUID` queda afuera; los patrones son ASCII, así que una `ñ` en un nombre se marca;
`WhitespaceAround` también marca las llaves vacías pegadas (`{}`), como `sun_checks`. `minimal`
no las pide.

## Inspecciones de IntelliJ

De las inspecciones de IntelliJ, kerbo hace:

- `SameParameterValue` (`same-parameter-value`): un parámetro al que todas las llamadas le pasan la
  misma constante, que se muestra como la muestra IntelliJ (`"ARS"`, `Precios.ESCALA`). Lo privado
  se mira por archivo; lo de paquete y protegido, sobre el proyecto entero (IntelliJ no marca lo
  público).
- `UnusedReturnValue` (`unused-return-value`): un método que devuelve algo que ninguna llamada usa.
- `SameReturnValue` (`same-return-value`): todos los `return` devuelven el mismo literal. En un
  método abstracto, todas sus implementaciones (también las clases anónimas); hace falta al menos
  una fuera de los tests, porque una proyección de Spring Data sólo tiene stubs de prueba y la
  implementación real la arma Spring. Una interfaz con un solo método abstracto se salta: puede
  tener implementaciones por lambda que no se ven.
- `unused` para métodos (`unused-method`): ninguna llamada, referencia (`::nombre`) ni texto
  (`"nombre"`, como en `@MethodSource`) del proyecto lo nombra, y ningún framework conocido lo
  llama. Los frameworks son tablas de datos en `src/index/java_unused.rs`: un método con una
  anotación que no está en la lista de las pasivas (`@Transactional`, `@Deprecated`, `@Nullable`…)
  se da por usado (así entran `@GetMapping`, `@Bean`, `@PrePersist`, `@Test`, `@JsonCreator` o
  una anotación propia), igual que los de un `@Mapper` de MapStruct o un `@Aspect`, los accessors
  (`get`/`is`/`set`, que usan Jackson y JPA) y los que llama el JDK por nombre. Lo público y lo
  protegido sólo se marca en una aplicación (`@SpringBootApplication`, `@QuarkusMain`,
  `@MicronautApplication`): en una librería es su API.
- `unused` para campos que se asignan pero nunca se leen (`unread-field`): un campo privado que el
  código de su archivo sólo escribe. Un parámetro o una variable local con el mismo nombre no lo
  lee (`this.total = total`); un `+=` o un `++`, sí. Las tablas de frameworks son las mismas de
  `unused-field`.
- `unused` para campos (`unused-field`): ningún código ni texto del proyecto nombra el campo (lo
  privado se mira en su archivo). Un texto que inicializa a una constante de su mismo nombre
  (`PAGADO = "PAGADO"`) no cuenta como uso. Las anotaciones de inyección (`@Autowired`, `@Inject`,
  `@Value`…) no salvan a un campo que nadie lee; cualquier otra que no sea pasiva sí (`@Column`,
  `@Mock`, `@MockitoBean`, `@JsonProperty`), y también una anotación de la clase que puede leer sus
  campos de instancia (`@Data`, `@Getter`, `@Entity`, una propia). Los estereotipos de Spring, las
  anotaciones de los tests y los constructores de Lombok no leen campos: un `private final` de un
  `@Service` con `@RequiredArgsConstructor` que nadie usa se marca.

- `NullableProblems` (`nullable-problems`): un método con `@Override` (también en una clase
  anónima) que redefine uno de una librería no nula por defecto (Spring 7 con `@NullMarked`,
  Spring 6 con `@NonNullApi`) y deja sin anotar un parámetro o el objeto que devuelve. Como
  IntelliJ, no marca el parámetro que el cuerpo desreferencia antes de cualquier `if` con salida
  o `try` (de ahí infiere que es no nulo), ni lo que la librería declara `@Nullable`, ni nada si
  el paquete del proyecto ya es `@NullMarked`; el retorno se marca aunque devuelva un literal.

- `DataFlowIssue`, el caso de las cadenas (`data-flow-issue`): una llamada sobre el resultado de
  un método que su librería declara `@Nullable`, en la misma expresión
  (`respuesta.getBody().error()`, `SecurityContextHolder.getContext().getAuthentication().getName()`).
  El tipo de cada paso sale de la firma del anterior en los jars. No marca si ese mismo resultado
  se chequeó antes en el método (`!= null`, `assertThat(...).isNotNull()`, `assertNotNull`,
  `requireNonNull`). También sigue variables: una asignada desde una llamada `@Nullable` y usada
  en un camino sin chequeo (`if (x == null) return;`, `x != null && ...`, `instanceof`, `hasText`,
  los bucles, `try`, sus recursos y `finally` se entienden como IntelliJ), y un resultado
  `@Nullable` que se desempaqueta (`return jdbc.queryForObject(...)` en un método que devuelve
  `long`). Con sobrecargas, un argumento `Long.class` elige las que reciben un `Class`, como el
  compilador; un varargs acepta su cantidad de parámetros o más. Del JDK, que no viene en los jars,
  hay una tabla propia (`src/index/java_jdk.rs`) con lo que IntelliJ da por posiblemente null:
  `getResourceAsStream` y `getResource`, `System.getProperty` y `getenv`, `File.getParentFile`,
  `getParent`, `listFiles` y `list`. Esa parte anda sin classpath. Un método del proyecto que
  devuelve `null` en algún `return` y que nadie puede redefinir (estático, privado, `final`, de una
  clase `final`) cuenta como `@Nullable`, como lo infiere IntelliJ, y la cadena sigue por el tipo
  que declara.
- `DataFlowIssue`, el caso de los argumentos (`null-argument`): un `null` (o `(Tipo) null`) pasado a
  un parámetro que el método desreferencia en todos los caminos, si nadie puede redefinirlo: de eso
  IntelliJ infiere que el parámetro es no nulo. Con sobrecargas, sólo si todas lo desreferencian.
  También una variable asignada desde una llamada que puede devolver `null` y pasada sin chequeo a
  un parámetro así; desde ahí IntelliJ la da por no nula, y no se marcan sus usos siguientes. Un
  `do` cuenta como cuerpo que corre al menos una vez.
- `DataFlowIssue`, el caso de la asignación que no cambia nada (`already-assigned`): en un mismo
  bloque, un literal asignado a una variable local, o a una posición fija de un arreglo local recién
  creado, que ya tenía ese valor (el anterior o el por defecto de `new T[n]`) sin que nada la usara
  en el medio.
- `DataFlowIssue`, el caso de la llamada que falla siempre (`failing-call`): una llamada cuyos
  argumentos `null`, `true` o `false` cumplen siempre las condiciones con que el método tira, según
  el contrato que IntelliJ infiere del cuerpo (un `if` que tira) o conoce de memoria (`Validate`,
  el `Assert` de Spring, las aserciones de JUnit y TestNG). En los tests, no dentro de un `try` con
  `catch`, de `assertThrows` ni de `@Test(expected = ...)`.
- `OptionalGetWithoutIsPresent` (`optional-get-without-is-present`): `get()` encadenado a
  `findFirst()`, `findAny()`, `max(...)`, `min(...)` u `Optional.ofNullable(...)`.

- `RedundantThrows` (`redundant-throws`): una excepción chequeada en el `throws` que el cuerpo no
  puede tirar, ni declara un método que lo redefine, ni atrapa un llamador con un `catch` de ese tipo.
  Lee las firmas de los jars y las del JDK del `release` del proyecto (`JAVA_HOME` o el `java` del
  `PATH`). Como IntelliJ, no opina de un método con una llamada que no sabe resolver. Sin JDK no avisa.

- `AutoCloseableResource` (`auto-closeable-resource`): una llamada que crea algo que se cierra
  (un `InputStream`, una `Connection`, un `ResultSet`, un `ExecutorService`, un `HttpClient`...)
  fuera de un `try` con recursos: en una variable que no se cierra, no se devuelve ni se pasa a
  otro método, o encadenada (`fabrica().getValidator()`). Qué se cierra sale de los jars, del tipo
  que declara un método del proyecto o de una tabla propia del JDK; lo que devuelve un `get...` es
  de quien lo da (el contenedor o el pool).

## Cómo lee las librerías

Para leer las librerías, kerbo le pide el classpath una sola vez, sin red, a Maven
(`mvn -o dependency:build-classpath`) o a Gradle (el `gradlew` del proyecto si está, con un script
de inicio propio que lista los jars de `testRuntimeClasspath`, `runtimeClasspath` y
`compileClasspath` y saltea lo que no está bajado), y lo guarda con la huella de los archivos de
build; de los jars lee sólo el índice y las clases que necesita, sin bytecode. Con otra
herramienta, la lista de jars va en `KERBO_JAVA_CLASSPATH` (separados como en el PATH). Sin
classpath, las reglas que leen las librerías sólo saben lo de la tabla propia del JDK.

Las que miran el proyecto entero sólo corren cuando el análisis lo abarca (`kerbo check` en la
raíz): sobre una carpeta o un archivo suelto faltarían llamadas. Sin compilar, kerbo resuelve el
receptor de una llamada cuando se puede leer: `repo.buscar()` con `Repositorio repo` declarado en
el archivo, `Clase.metodo()`, o `metodo()` desde adentro de una clase (con lo que hereda por
`extends` e `implements`). Una cadena de llamadas, una lambda sin tipos o un `import static`
quedan como desconocidos y cuentan para todos; con dos métodos iguales en la misma clase o en
clases emparentadas no marca nada. En una librería (sin `@SpringBootApplication` o similar), lo
público y lo protegido es API y no se revisa quién lo llama.
