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
`InterfaceIsType`, `MissingSwitchDefault`, `FallThrough`, `NeedBraces`, `EmptyBlock`,
`AvoidNestedBlocks`, `InnerAssignment`, `SimplifyBooleanExpression`, `SimplifyBooleanReturn`,
`EqualsHashCode`, `HideUtilityClassConstructor`,
`TodoComment`, `IllegalImport`, `FileTabCharacter`, los espacios al final de línea
(`trailing-spaces`, el `RegexpSingleline` de `sun_checks`), `NewlineAtEndOfFile`,
`DesignForExtension`, `HiddenField`, `VisibilityModifier`, `DeclarationOrder`,
`LocalVariableName`, `LocalFinalVariableName` y los de espacios y llaves: `WhitespaceAround`, `WhitespaceAfter`,
`NoWhitespaceBefore`, `NoWhitespaceAfter`, `GenericWhitespace`, `MethodParamPad`,
`EmptyForIteratorPad`, `ParenPad`, `LeftCurly`, `RightCurly` y `OperatorWrap`. Se arreglan solos los espacios
al final, `NewlineAtEndOfFile` y los de espacios (no las llaves ni `OperatorWrap`, que piden
reacomodar líneas). Siguen a Checkstyle en lo que no se ve: `MethodName` no mira lo que lleva
`@Override` y marca el método que se llama como su clase; un campo de interfaz es una constante;
`serialVersionUID` queda afuera; los patrones son ASCII, así que una `ñ` en un nombre se marca;
`WhitespaceAround` también marca las llaves vacías pegadas (`{}`), como `sun_checks`;
`FallThrough` toma por terminado un `case` como `CheckUtil.isTerminated` (un `if` sin `else` no
termina, un `while (true)` sin `break` sí) y sólo acepta el comentario `fall through` en
minúsculas (`//$FALL-THROUGH$` no cuenta); `HiddenField` toma los componentes de un record como
campos; `LeftCurly` pide salto de línea después de la llave de cualquier cuerpo de clase (también
la anónima y la de una constante de enum) o de `switch` si el primer miembro sigue en la misma
línea, salvo en el cuerpo de un enum y en una anónima que empieza con un bloque de inicialización;
`OperatorWrap` mira el token de al lado tal cual: con `&& (` al final de la línea no avisa;
`ParenPad` no mira los paréntesis de un cast; `VisibilityModifier` deja pasar `@Rule`,
`@ClassRule` y `@VisibleForTesting`, pero no `@TempDir` (Checkstyle 14 la sacó de la lista).
`minimal` no las pide.

Las de Javadoc de `sun_checks.xml` sólo las pide `strict`: `JavadocVariable` (`javadoc-variable`),
`MissingJavadocMethod`, `JavadocMethod`, `JavadocType`, `JavadocPackage` e
`InvalidJavadocPosition`. kerbo lee las etiquetas de bloque como el árbol de Javadoc de Checkstyle:
una arroba al principio de la línea y fuera de un tag inline, así que un `@Override` adentro de
`{@snippet : ...}` no es una etiqueta. `JavadocPackage` se avisa una vez por carpeta sin
`package-info.java`, en el primer archivo por orden alfabético, y `module-info.java` no cuenta. Un
varargs con anotación antes de los puntos (`Object @Nullable ... args`) se lee aunque tree-sitter no
lo entienda. Cuando el HTML de un Javadoc no entra en la gramática de Checkstyle (un `<N, E>` que
toma por etiqueta, un `</p>` que no cierra nada, una etiqueta que se cierra con otro nombre),
`JavadocMethod` y `JavadocType` avisan el error en su línea y no revisan las etiquetas de ese
Javadoc, como Checkstyle: kerbo calca su lexer y su parser de HTML (la etiqueta sin cierre no puede
tener elementos adentro; gana el primer error del parser y, si no hay, el último del lexer).

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
- También sobran, como para PMD: un `import static a.B.C;` de un tipo que sólo se usa como tipo, un
  `import a.b.Clase.*;` (comodín sobre una clase y no un paquete) y el import de un tipo anidado
  (`java.util.Map.Entry`) cuando en cada uso el nombre ya llega como tipo miembro de una clase que
  lo encierra o de sus padres (del proyecto, de las dependencias o del JDK). Si en el camino hay un
  supertipo que PMD no puede resolver como está escrito, o el Javadoc nombra el tipo, cuenta como
  usado.
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

Las que dependen de la versión de Java usan el `release` de cada módulo, como el nivel de
lenguaje por módulo de IntelliJ: el del `pom.xml` o `build.gradle` de la carpeta, o el del módulo
que la contiene, y si nadie lo dice, el del JDK. Jenkins compila `war/` con 11 y el resto con 21;
Maven dice el suyo en `<javaVersion>`, que el pom padre de Apache pasa a `maven.compiler.release`.

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
- `DataFlowIssue`, el `@Nullable` de más (`redundant-nullable`): un método que nadie puede
  redefinir (estático, privado, `final`, o de una clase `final`, anónima o record), anotado
  `@Nullable` o `@CheckForNull`, cuyos `return` devuelven todos algo que nunca es null: un literal,
  un `new`, `this`, una operación, o una local asignada una sola vez con algo así. No cuentan los
  que devuelven un arreglo o `Void`, ni los que tienen una lambda en el cuerpo.
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
  `StringBuilder` y `StringBuffer` desde 15), salvo adentro de la clase de ese mismo nombre. Si el
  receptor es lo que devuelve una cadena de llamadas (`pex.getResults().get(0).getProblems().size() >
  0`), el índice la sigue con los tipos genéricos del proyecto y de las dependencias, como hace con
  los `throws`. Coincide con Qodana en el corpus (34).

- `ManualMinMaxCalculation` (`manual-min-max-calculation`): un mínimo o un máximo escrito a mano
  con un ternario (`a < b ? a : b`), un `if`/`else` que asigna o devuelve uno de los dos lados, o
  un `if` sin `else` que pisa lo que asignó, declaró o devuelve la sentencia de al lado. Se arregla
  con `Math.min` o `Math.max`. Los dos lados tienen que ser del mismo tipo numérico (`int`, `long`,
  `float` o `double`: variables con tipo declarado o literales) y no aplica dentro de la propia
  `java.lang.Math`. Coincide con Qodana en el corpus (2).

- `DefaultAnnotationParam` (`default-annotation-param`): un elemento de una anotación escrito con
  el mismo valor que ya tiene por defecto (`@Reintento(intentos = 3)` con `intentos() default 3`).
  El valor por defecto sale de la anotación si la declara el proyecto (si el nombre puede ser de
  más de una, no se reporta) y si no, del `AnnotationDefault` del `.class` en las dependencias,
  como `@Argument(index = 0)` de args4j. Coincide con Qodana en el corpus (17: 7 en junit4 y 10 en
  Jenkins).

- `Java8MapApi` (`java8-map-api`), en la forma de `computeIfAbsent`: un valor que se lee con `get`,
  se calcula si es `null` y se guarda con `put`. Desde Java 8, cuando el tipo declarado es un `Map`
  y lo calculado no puede tener efectos (un arreglo o un literal, sin llamadas): con efectos,
  IntelliJ lo deja como información. Coincide con Qodana en el corpus (2, en commons-lang).

- `TextBlockMigration` (`text-block-migration`): literales de texto concatenados con dos o más
  `
` escritos como escape, donde un bloque de texto se lee mejor. Desde Java 15, sólo si todas las
  partes son literales y no hay comentarios en el medio. Coincide con Qodana en el corpus (1).

- `SequencedCollectionMethodCanBeUsed` (`sequenced-collection-method`): `get(0)`, `remove(0)`,
  `add(0, x)`, `get(size() - 1)` e `iterator().next()` donde Java 21 tiene `getFirst()`,
  `getLast()`, `removeFirst()` y `addFirst()`. El tipo sale de la declaración: una variable, un
  campo, el campo de una clase del mismo archivo o lo que devuelve un método propio. Si el tipo
  declarado no es una de las colecciones conocidas, se busca en las dependencias si alguno de sus
  padres lo es (un `DomNodeList` de htmlunit es una `List`). No avisa en una serie como `get(0)`,
  `get(1)`, igual que IntelliJ. Coincide con Qodana en el corpus (10, en Jenkins).

- `MathClampMigration` (`math-clamp-migration`): `Math.max(0, Math.min(60, x))` donde Java 21 tiene
  `Math.clamp`. Como el modo por defecto de IntelliJ: alcanza con que algo dentro de la llamada
  interna no sea constante. Coincide con Qodana en el corpus (2).

- `StringRepeatCanBeUsed` (`string-repeat-can-be-used`): desde Java 21,
  `builder.append(texto.repeat(n))` sobre un `StringBuilder` o `StringBuffer`; desde Java 11, un
  `for` que cuenta con un `int` (`i < n`, `<=` o `!=`, `i++`, o al revés bajando) y en cada vuelta
  sólo agrega lo mismo al builder, sin usar el contador ni llamar a nada. Coincide con Qodana en el
  corpus (3) y en el proyecto de prueba (6).

- `PatternVariableCanBeUsed` (`pattern-variable-can-be-used`), desde Java 16: una variable local
  que se inicializa con un cast (`Foo foo = (Foo) valor;`) que un `valor instanceof Foo` anterior ya
  garantiza, y un cast suelto donde el instanceof ya declara la variable (`valor instanceof Foo f`).
  El instanceof se busca como IntelliJ: el `&&` o el ternario que encierra el cast, la condición del
  if o del bucle que encierra la sentencia, o un if anterior del mismo bloque que sale cuando no se
  cumple; si lo casteado se reasigna en el medio, no vale. Coincide con Qodana en el corpus (5).

- `EnhancedSwitchMigration` (`enhanced-switch-migration`), desde Java 14, en la forma que avisa por
  defecto: un switch viejo que devuelve o tira en todas sus ramas (de hasta dos sentencias), con
  `default` o con un `return` o `throw` después, que se puede escribir como `return switch`; o que
  en cada rama asigna la misma variable local (o tira), con `default` o con la variable ya asignada
  antes. Coincide con Qodana en el corpus (2) y en el proyecto de prueba
  `scripts/parity/java/fixtures/api-nueva` (5 de 10 casos).

- `Java9CollectionFactory` (`java9-collection-factory`): `Collections.unmodifiableList(new
  ArrayList<>(otra))` donde Java 10 tiene `List.copyOf(otra)` (igual con `Set` y `Map`), y
  `Collections.unmodifiableList(Arrays.asList(a, b))` donde alcanza `List.of(a, b)` si ningún
  elemento puede ser null. Un `ArrayList` copiado sólo compila con una colección o un int, así que
  basta con que lo copiado no sea un número; en un `Map`, el origen tiene que ser `Map`,
  `AbstractMap` o `HashMap`. También la colección armada a mano: una variable local creada vacía (o
  con capacidad) seguida sólo de `add` o `put` hasta el `unmodifiable...`, sin otros usos, y la
  doble llave; los valores de enum y las constantes cuentan como no null. Coincide con Qodana en el
  corpus (19) y en el proyecto de prueba (7 de 11 casos).

- `ReadWriteStringCanBeUsed` (`read-write-string`): `new String(Files.readAllBytes(ruta), juego)`
  donde Java 11 tiene `Files.readString`, y `Files.write(ruta, texto.getBytes(juego))` donde Java 12
  tiene `Files.writeString`, cuando el juego de caracteres es un `Charset`. Coincide con Qodana en el
  corpus (5).

- `FinalStaticMethod` (`final-static-method`): `final` en un método `static`, que no se redefine;
  sólo impide que una subclase declare otro con la misma firma. Se arregla sacando el `final`.
  Coincide con Qodana en el corpus (23).

- `transient` en un campo `static` (`transient-static-field`, la parte de `UnnecessaryModifier`
  que no tienen Checkstyle ni PMD): la serialización nunca guarda un campo `static`. Se arregla
  sacando el `transient`. Coincide con Qodana en el corpus (2, en commons-lang).

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
