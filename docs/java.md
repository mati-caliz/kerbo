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

Las de Javadoc de `sun_checks.xml` sólo las pide `strict`: `JavadocVariable` (`javadoc-variable`),
`MissingJavadocMethod`, `JavadocMethod`, `JavadocType`, `JavadocPackage` e
`InvalidJavadocPosition`. kerbo lee las etiquetas de bloque como el árbol de Javadoc de Checkstyle:
una arroba al principio de la línea y fuera de un tag inline, así que un `@Override` adentro de
`{@snippet : ...}` no es una etiqueta. `JavadocPackage` se avisa una vez por carpeta sin
`package-info.java`, en el primer archivo por orden alfabético, y `module-info.java` no cuenta. Un
varargs con anotación antes de los puntos (`Object @Nullable ... args`) se lee aunque tree-sitter no
lo entienda. Lo único que no se imita son los Javadoc que Checkstyle no puede parsear (un `<N, E>`
suelto que toma por HTML): Checkstyle avisa el error de parseo con el nombre de la regla y kerbo no.

## PMD

`UnusedImports`, `RedundantImport`, `RedundantModifier` y `FinalClass` tienen su par en el
quickstart de PMD 7 (`UnnecessaryImport`, que junta los dos primeros, `UnnecessaryModifier` y
`ClassWithOnlyPrivateConstructorsShouldBeFinal`), pero no avisan lo mismo, así que esas reglas
imitan a uno o al otro según `[java] reference` de `kerbo.toml`. Si falta, kerbo imita a PMD cuando el build lo usa y no usa Checkstyle
(`maven-pmd-plugin` en el `pom.xml`, el plugin `pmd` de Gradle, una carpeta `config/pmd` o un
`pmd*.xml` en la raíz), y a Checkstyle en cualquier otro caso. Imitando a PMD:

- Un import que sólo nombra el Javadoc (`{@link}`, `{@linkplain}`, `{@value}`, `@see`, `@throws`,
  `@exception`, los tipos de los parámetros de `#metodo(...)`, el `target` de un `@link` en un
  snippet y los links de un comentario `///`) está usado, y uno de `java.lang` que se usa no se
  marca como sin usar (sí como redundante).
- No se marca el `public` del constructor de una clase que no es pública, ni `strictfp`, ni el
  `final` de los parámetros de un método abstracto o de una variable `_`.
- `final` sobra en un método de un enum sin constantes con cuerpo y en el de un record; el `public`
  de un record adentro de una interfaz no se marca.
- Una clase abstracta con sólo constructores privados también debería ser `final`. Las subclases se
  buscan por tipo: cuenta una anónima en cualquier expresión y `Generic<String>.Inner`, y una
  clase local tapa a la miembro del mismo nombre. `@Value` o un constructor de Lombok que no sea
  privado la dejan afuera.
- El aviso va en la línea del nombre de la declaración y no en la de su primera anotación, y
  `@SuppressWarnings("all")`, `"PMD"` o `"PMD.<regla>"` y un `// NOPMD` lo callan.

junit4, commons-lang y guava (sin `android/`) coinciden con PMD 7.29 en los modificadores y las
clases finales. En los imports faltan los que PMD sabe innecesarios porque resuelve tipos: un `*`
del que no se usa nada y un tipo anidado que ya llega heredado (2 en junit4, 11 en guava).

Contra Qodana, imitando a PMD, los imports coinciden y en los modificadores faltan sólo dos
inspecciones que no tienen ni Checkstyle ni PMD: `static` y `final` juntos en un método
(`FinalStaticMethod`) y `transient` en un campo estático.

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
- `DataFlowIssue`, el retorno nulo (`nullable-return`): un método cuyo retorno es no nulo por
  `@NullMarked` (del paquete, la clase o las que la encierran) o por `@NonNull` que devuelve un
  `null` escrito, una variable o campo `@Nullable` sin chequear o una llamada que puede devolver
  null. No cuentan los primitivos, `void`/`Void`, los retornos `@Nullable` o `@ParametricNullness`
  ni las variables de tipo con límite `@Nullable`.
- `OptionalGetWithoutIsPresent` (`optional-get-without-is-present`): `get()` encadenado a
  `findFirst()`, `findAny()`, `max(...)`, `min(...)` u `Optional.ofNullable(...)`.

- `RedundantThrows` (`redundant-throws`): una excepción chequeada en el `throws` que el cuerpo no
  puede tirar, ni declara un método que lo redefine, ni atrapa un llamador con un `catch` de ese tipo.
  Lee las firmas de los jars y las del JDK del `release` del proyecto (`JAVA_HOME` o el `java` del
  `PATH`). Como IntelliJ, no opina de un método con una llamada que no sabe resolver. Sin JDK no avisa.

- `UnnecessaryUnicodeEscape` (`unnecessary-unicode-escape`): un `\u00e9` de un carácter que se
  puede escribir tal cual, en cualquier parte del archivo (también en comentarios), y se arregla
  escribiéndolo (un tab adentro de un string queda `\t`). Quedan los controles, los espacios
  que no son el común, los invisibles, los sin asignar, los separadores de línea, las marcas
  combinantes y U+FFFD; las categorías son las del JBR 25 que trae Qodana (Unicode 16). Tampoco se
  marca lo que la codificación del archivo no puede escribir: la del proyecto
  (`project.build.sourceEncoding` o `options.encoding`, o `[java] encoding`), salvo que el archivo
  ya traiga UTF-8 fuera de ASCII, que entonces se lee como UTF-8, igual que en IntelliJ.
  commons-lang (que declara ISO-8859-1) y guava coinciden con Qodana en los 23.120.

- `Java8ListSort` (`java8-list-sort`): `Collections.sort(lista, comparador)` con `release` 8 o más,
  y se arregla como `lista.sort(comparador)`. Cuenta el `sort` importado estático salvo que la clase
  declare su propio `sort`, o que también se importe `Arrays.sort` y el primer argumento sea un
  arreglo (declarado como `T[]`, un `new T[]` o un `toArray()`). Coincide con Qodana en guava (31).

- `SizeReplaceableByIsEmpty` (`size-replaceable-by-is-empty`): `size()` o `length()` comparado con
  0 o 1 de una forma que pregunta si está vacío (`== 0`, `< 1`, `> 0`, `!= 0`, `>= 1`, en cualquier
  orden), y se arregla con `isEmpty()` o `!isEmpty()`. El número puede ser una constante `final` del
  alcance. Sin resolver tipos: el receptor es una variable cuyo tipo declarado tiene `isEmpty()`
  (las colecciones y mapas de `java.util` y de guava; `String` desde `release` 6 y `CharSequence`,
  `StringBuilder` y `StringBuffer` desde 15), salvo adentro de la clase de ese mismo nombre.
  Coincide con Qodana en el corpus (30).

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
