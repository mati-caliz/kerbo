# Java

## Checkstyle

The six Java form rules mimic Checkstyle 14 with `sun_checks.xml`, the reference configuration
Checkstyle ships and the one already used for `MagicNumber` and the lengths: `UnusedImports` (with
`processJavadoc` off, as in `sun_checks`), `RedundantImport`, `AvoidStarImport`,
`RedundantModifier`, `FinalParameters` and `FinalClass`. `google_checks.xml` only asks for
`AvoidStarImport`, and PMD 7's quickstart has the equivalents for imports, modifiers and final
classes but not `final` parameters. `recommended` does not ask for `final` parameters and `minimal`
only checks unused and duplicated imports. Columns count bytes, as in every other rule: with tabs,
Checkstyle expands them to 8 and reports a different number.

From the rest of `sun_checks.xml`, kerbo does the names (`TypeName`, `MethodName`, `ConstantName`,
`StaticVariableName`, `MemberName` and `ParameterName`, with the same rule names in kebab case:
`type-name`, `method-name`...), `UpperEll` (`upper-ell`, fixed automatically), `EmptyStatement`
(`empty-statement`), `ModifierOrder`, `ArrayTypeStyle`, `MultipleVariableDeclarations`,
`InterfaceIsType`, `MissingSwitchDefault`, `NeedBraces`, `EmptyBlock`, `AvoidNestedBlocks`,
`InnerAssignment`, `SimplifyBooleanExpression`, `EqualsHashCode`, `HideUtilityClassConstructor`,
`TodoComment`, `IllegalImport`, `FileTabCharacter`, trailing spaces (`trailing-spaces`, the
`RegexpSingleline` in `sun_checks`), `NewlineAtEndOfFile`, `DesignForExtension`, `HiddenField`,
`VisibilityModifier`, `DeclarationOrder`, `LocalFinalVariableName` and the whitespace and brace
ones: `WhitespaceAround`, `WhitespaceAfter`, `NoWhitespaceBefore`, `NoWhitespaceAfter`,
`GenericWhitespace`, `MethodParamPad`, `EmptyForIteratorPad`, `LeftCurly`, `RightCurly` and
`OperatorWrap`. Trailing spaces, `NewlineAtEndOfFile` and the whitespace ones are fixed
automatically (not the braces nor `OperatorWrap`, which need lines rearranged). They follow
Checkstyle where it is not obvious: `MethodName` skips what carries `@Override` and flags the
method named like its class; an interface field is a constant; `serialVersionUID` is left out;
the patterns are ASCII, so an `ñ` in a name is flagged; `WhitespaceAround` also flags empty braces
written together (`{}`), as `sun_checks` does. `minimal` does not ask for them.

## IntelliJ inspections

From IntelliJ's inspections, kerbo does:

- `SameParameterValue` (`same-parameter-value`): a parameter that every call passes the same
  constant, shown the way IntelliJ shows it (`"USD"`, `Prices.SCALE`). Private code is checked
  per file; package-private and protected code, over the whole project (IntelliJ does not flag
  public code).
- `UnusedReturnValue` (`unused-return-value`): a method that returns something no call uses.
- `SameReturnValue` (`same-return-value`): every `return` gives back the same literal. For an
  abstract method, all its implementations (anonymous classes too); at least one has to be outside
  tests, because a Spring Data projection only has test stubs and Spring builds the real one. An
  interface with a single abstract method is skipped: it may have lambda implementations that
  cannot be seen.
- `unused` for methods (`unused-method`): no call, reference (`::name`) or string (`"name"`, as in
  `@MethodSource`) in the project names it, and no known framework calls it. Frameworks are data
  tables in `src/index/java_unused.rs`: a method with an annotation outside the list of passive
  ones (`@Transactional`, `@Deprecated`, `@Nullable`…) counts as used (so `@GetMapping`, `@Bean`,
  `@PrePersist`, `@Test`, `@JsonCreator` or a custom annotation do), as do the methods of a
  MapStruct `@Mapper` or an `@Aspect`, accessors (`get`/`is`/`set`, used by Jackson and JPA) and
  the ones the JDK calls by name. Public and protected code is only flagged in an application
  (`@SpringBootApplication`, `@QuarkusMain`, `@MicronautApplication`): in a library it is its API.
- `unused` for fields that are assigned but never read (`unread-field`): a private field the code in
  its file only writes. A parameter or local variable with the same name does not read it
  (`this.total = total`); a `+=` or a `++` does. The framework tables are the ones of
  `unused-field`.
- `unused` for fields (`unused-field`): no code or string in the project names the field (private
  fields are checked in their own file). A string that initializes a constant of the same name
  (`PAID = "PAID"`) does not count as a use. Injection annotations (`@Autowired`, `@Inject`,
  `@Value`…) do not save a field nobody reads; any other non-passive one does (`@Column`, `@Mock`,
  `@MockitoBean`, `@JsonProperty`), and so does a class annotation that may read its instance
  fields (`@Data`, `@Getter`, `@Entity`, a custom one). Spring stereotypes, test annotations and
  Lombok constructors do not read fields: an unused `private final` in a `@Service` with
  `@RequiredArgsConstructor` is flagged.

- `NullableProblems` (`nullable-problems`): an `@Override` method (also in an anonymous class)
  overriding one from a library that is non-null by default (Spring 7 with `@NullMarked`, Spring 6
  with `@NonNullApi`) that leaves a parameter or the returned object unannotated. Like IntelliJ,
  it does not flag a parameter the body dereferences before any exiting `if` or a `try` (from
  that it infers non-null), what the library declares `@Nullable`, or anything if the project
  package is already `@NullMarked`; the return is flagged even if it returns a literal.

- `DataFlowIssue`, the chain case (`data-flow-issue`): a call on the result of a method its
  library declares `@Nullable`, in the same expression (`response.getBody().error()`,
  `SecurityContextHolder.getContext().getAuthentication().getName()`). Each step's type comes from
  the previous one's signature in the jars. It is not flagged if that same result was checked
  earlier in the method (`!= null`, `assertThat(...).isNotNull()`, `assertNotNull`,
  `requireNonNull`). It also follows variables: one assigned from a `@Nullable` call and used on a
  path without a check (`if (x == null) return;`, `x != null && ...`, `instanceof`, `hasText`,
  loops, `try`, its resources and `finally` are understood as IntelliJ does), and a `@Nullable`
  result that gets unboxed (`return jdbc.queryForObject(...)` in a method returning `long`). With
  overloads, a `Long.class` argument picks the ones taking a `Class`, as the compiler does; a
  varargs method accepts its parameter count or more. For the JDK, which does not come in the jars,
  there is kerbo's own table (`src/index/java_jdk.rs`) of what IntelliJ treats as possibly null:
  `getResourceAsStream` and `getResource`, `System.getProperty` and `getenv`, `File.getParentFile`,
  `getParent`, `listFiles` and `list`. That part works without a classpath. A project method that
  returns `null` in some `return` and that nothing can override (static, private, `final`, in a
  `final` class) counts as `@Nullable`, as IntelliJ infers it, and the chain follows its declared
  type.
- `DataFlowIssue`, the argument case (`null-argument`): a `null` (or `(Type) null`) passed to a
  parameter the method dereferences on every path, if nothing can override it: from that IntelliJ
  infers the parameter is non-null. With overloads, only if all of them dereference it. Also a
  variable assigned from a call that may return `null` and passed unchecked to such a parameter;
  from there on IntelliJ treats it as non-null, and its later uses are not reported. A `do` body
  counts as running at least once.
- `DataFlowIssue`, the assignment that changes nothing (`already-assigned`): within one block, a
  literal assigned to a local variable, or to a fixed position of a freshly created local array,
  that already held that value (the previous one or the default of `new T[n]`) with nothing using
  it in between.
- `DataFlowIssue`, the call that always fails (`failing-call`): a call whose `null`, `true` or
  `false` arguments always meet the conditions under which the method throws, by the contract
  IntelliJ infers from its body (an `if` that throws) or knows by heart (`Validate`, Spring's
  `Assert`, the JUnit and TestNG assertions). In tests, not inside a `try` with `catch`,
  `assertThrows` or `@Test(expected = ...)`.
- `OptionalGetWithoutIsPresent` (`optional-get-without-is-present`): `get()` chained to
  `findFirst()`, `findAny()`, `max(...)`, `min(...)` or `Optional.ofNullable(...)`.

- `RedundantThrows` (`redundant-throws`): a checked exception in the `throws` clause that the body
  cannot throw, that no overriding method declares and that no caller catches with a `catch` of that
  type. It reads the signatures of the jars and of the JDK for the project's `release` (`JAVA_HOME` or
  the `java` in `PATH`). Like IntelliJ, it has no opinion on a method with a call it cannot resolve.
  Without a JDK it reports nothing.

- `AutoCloseableResource` (`auto-closeable-resource`): a call that creates something closeable (an
  `InputStream`, a `Connection`, a `ResultSet`, an `ExecutorService`, an `HttpClient`...) outside a
  try-with-resources: in a variable that is not closed, returned or passed to another method, or
  chained (`factory().getValidator()`). What is closeable comes from the jars, from the type a
  project method declares, or from kerbo's own JDK table; what a `get...` returns belongs to whoever
  gives it (the container or the pool).

## How it reads libraries

To read libraries, kerbo asks for the classpath once, offline, to Maven
(`mvn -o dependency:build-classpath`) or to Gradle (the project's `gradlew` when there is one, with
kerbo's own init script that lists the jars of `testRuntimeClasspath`, `runtimeClasspath` and
`compileClasspath` and skips what is not downloaded), and stores it with the fingerprint of the
build files; from the jars it reads only the index and the classes it needs, no bytecode. With any
other tool, the list of jars goes in `KERBO_JAVA_CLASSPATH` (separated as in PATH). Without a
classpath, the rules that read libraries only know kerbo's own JDK table.

The ones that look at the whole project only run when the analysis covers it (`kerbo check` at the
root): over a folder or a single file, calls would be missing. Without compiling, kerbo resolves a
call's receiver when it can be read: `repo.find()` with `Repository repo` declared in the file,
`Type.method()`, or `method()` from inside a class (with what it inherits through `extends` and
`implements`). A call chain, an untyped lambda or a `static import` stay unknown and count for
every candidate; with two equal methods in the same class or in related classes nothing is
flagged. In a library (without `@SpringBootApplication` or similar), public and protected code
is API and who calls it is not checked.
