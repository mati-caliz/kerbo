# kerbo

Español · [English](README.en.md)

Analizador de calidad de código para **Java, TypeScript/JavaScript, Python, Go y Rust** en un solo
binario. Aplica las mismas reglas en todos los lenguajes, en segundos y con pocos megas de
memoria, y cuenta igual que la herramienta que ya usás en cada uno (ESLint, ruff, Checkstyle y
PMD, golangci-lint, clippy). También revisa los scripts de shell como ShellCheck, las
migraciones de Postgres como squawk, los Dockerfile como hadolint y los archivos de docker
compose.

```console
$ kerbo check
src/billing/InvoiceService.java:88:5: cyclomatic-complexity La función `settle` tiene complejidad ciclomática 11 (máximo 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity La función `applyPromotions` tiene complejidad cognitiva 19 (máximo 15)
kerbo: 5810 archivos, 2 problemas, 2250 ms
```

## Instalación

Linux y macOS, en x86_64 y ARM (en Linux, un binario estático que corre en cualquier distro), y
Windows desde el bash de Git:

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

En Windows, x86_64 o ARM, también desde PowerShell:

```powershell
irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex
```

El instalador baja la última versión de los
[releases](https://github.com/mati-caliz/kerbo/releases) para la máquina, verifica el sha256 y
deja `kerbo` en `~/.local/bin` (el de PowerShell, en `%LOCALAPPDATA%\Programs\kerbo`, y la suma
al `PATH`; otra carpeta con `KERBO_INSTALL_DIR`).

Con Homebrew o con Rust instalado:

```sh
brew install mati-caliz/tap/kerbo
cargo install kerbo --locked
```

## Uso

```sh
kerbo check                       # el directorio actual
kerbo check api web               # varias carpetas o archivos
kerbo check --language java api   # sólo una pila: typescript, python, java o go
kerbo check --diff main           # sólo lo que tocan las líneas cambiadas desde main
kerbo check --format sarif        # también json, agent, agent-json; sarif lo leen GitHub y los editores
kerbo check --no-cache            # analiza todo de cero, sin leer ni escribir la caché
kerbo check --osv                 # suma las vulnerabilidades de las dependencias (consulta OSV)
kerbo osv-download                # baja la base de OSV para usar --osv=offline sin red
kerbo baseline                    # acepta lo que ya hay: desde ahora sólo se reporta lo nuevo
kerbo fix                         # aplica de una vez todos los arreglos mecánicos
kerbo fix --dry-run --rule unused-import src  # sólo muestra el parche, de una regla y una carpeta
kerbo explain cognitive-complexity  # por qué existe una regla y cómo se arregla
```

Respeta `.gitignore` y saltea los archivos y carpetas ocultos, salvo `.github/` y `.devcontainer/`,
que suelen tener Dockerfile, compose y scripts propios del repo; los demás ocultos (`.env`,
`.npmrc`) sólo se revisan buscando secretos. Sale con `0` si no hay problemas, `1` si los hay y
`2` si algo no se pudo leer o la configuración es inválida.

`kerbo check`, `kerbo baseline` y el servidor MCP guardan lo que miden de cada archivo en una
caché por contenido, así la corrida siguiente sólo analiza lo que cambió (y lo que se compara entre
archivos, como los bloques duplicados, se recombina con lo guardado). Vive en la carpeta de caché
del usuario (`~/.cache/kerbo`, `~/Library/Caches/kerbo` o `%LOCALAPPDATA%\kerbo`; otra con
`KERBO_CACHE_DIR`), se comparte entre proyectos y clones, ocupa como mucho 256 MB (otro tope con
`max_megabytes` en la sección `[cache]` de `kerbo.toml`, o con `KERBO_CACHE_MAX_MB`, que manda
sobre el archivo) y se descarta sola cuando cambia el binario o la configuración que afecta a un
archivo. Dos corridas a la vez no se pisan: cada parte de la caché se escribe con un lock del
sistema y releyendo lo que dejó la otra. Si se rompe, se ignora.

Los mensajes salen en español o en inglés. Manda `--lang en` (o `es`), después `lang` en
`kerbo.toml` y después el locale del sistema (`LC_ALL`, `LC_MESSAGES`, `LANG`); sin nada de eso,
español. El idioma sólo cambia el texto para personas: los identificadores de reglas, los `id`
estables y la forma del JSON, de SARIF y de `--format agent` y `agent-json` son los mismos.

### Arreglar de una vez

`kerbo fix` aplica todos los arreglos mecánicos (un import que sobra, un modificador de más, un
`final` que falta) en una sola corrida. Entre los arreglos de un archivo elige los que no se pisan
(dos que tocan el mismo rango no van juntos), los aplica, vuelve a analizar el texto arreglado y
repite hasta que no quede ninguno, así un arreglo que destapa otro (una clase que pasa a `final`
deja de más el `final` de sus métodos) también entra; los que se siguen pisando después de diez
vueltas quedan avisados. Después compara lo que queda con lo que había: si aparece un problema que
antes no estaba, ese archivo no se escribe y se lista lo nuevo. Cada archivo se escribe entero a un
temporal en la misma carpeta que después se renombra, así nunca queda a medio escribir.

`--rule` (se puede repetir) limita los arreglos a esas reglas y las rutas, a esas carpetas o
archivos. `--dry-run` (o `--diff`) no escribe nada y muestra el parche en diff unificado sin
contexto (`git apply --unidiff-zero`, también con CRLF). Como ruff, los arreglos que pueden cambiar lo
que hace el código (borrar una variable sin usar cuyo valor tiene efectos, `x == True` que pasa a `x`)
sólo se aplican con `--unsafe-fixes`; el "arreglar todo" del LSP tampoco los toma. `--since <referencia>` arregla sólo los
problemas de las líneas que cambiaron desde esa referencia de git. Una vuelta de arreglos que deja
errores de sintaxis nuevos no se acepta. Sale con `0` si todo quedó escrito, `1` si algún archivo
no se escribió porque el arreglo dejaba algo nuevo y `2` si algo no se pudo leer o escribir.

### Adoptarlo en un repo que ya existe

`kerbo baseline` guarda los problemas actuales en `kerbo-baseline.json` (commitealo). Desde ahí,
`kerbo check` sólo reporta los problemas nuevos y los que **empeoraron**: si una función de la
línea de base pasa de complejidad 14 a 16, vuelve a aparecer. Cada problema se identifica por
regla, archivo y función, no por número de línea, así que mover código no lo hace "nuevo".
`--no-baseline` muestra todo.

### Antitrampa

Con `--diff`, kerbo también mira **cómo** se resolvió un problema. Marca lo que un cambio agrega
para zafar en vez de arreglar, sólo en las líneas nuevas:

| Regla | Qué detecta |
|---|---|
| `suppression-added` | `eslint-disable`, `@ts-ignore`, `noqa`, `type: ignore`, `nolint`, `#nosec`, `@SuppressWarnings`, `NOPMD`, `CHECKSTYLE:OFF`, `istanbul ignore`, `NOSONAR`, `#[allow]`, `#[expect]`…, y un umbral aflojado en la configuración: un máximo que sube, un mínimo o una cobertura que baja, una regla que pasa a `off` o a `allow`, `strict` que pasa a `false` o un preset más laxo |
| `test-disabled` | `it.skip`, `it.only`, `xit`, `describe.only`, `@Disabled`, `@Ignore`, `@pytest.mark.skip`, `t.Skip()`, `#[ignore]`…, y un test que queda vacío |
| `test-removed` | tests borrados, comparando contra la referencia; si los tests sólo se movieron de archivo, no avisa |
| `type-escape` | `any` y `as unknown as` en TypeScript |

Todo se detecta sobre el árbol de sintaxis: un `eslint-disable` dentro de un string no cuenta.
También cuentan como supresión nueva los comentarios de las herramientas que kerbo reemplaza:
`-- squawk-ignore` y `-- squawk-ignore-file` en las migraciones, `# hadolint ignore=` y
`# hadolint global ignore=` en los Dockerfile, y `# shellcheck disable=`. Si una excepción es
legítima, va en `kerbo.toml` con su `reason`, a la vista de todos.

Esos comentarios, además, se respetan al reportar como lo hace la herramienta original: el
problema de las reglas nombradas en la línea siguiente (o en todo el archivo, con `-file` y
`global`) no se cuenta. Lo que sigue a la lista de reglas es el motivo
(`# hadolint ignore=DL3008 # la imagen base fija las versiones`), y cada problema ignorado sale en
el log con su motivo, o `sin motivo`, igual que los desvíos de `kerbo.toml`; el resumen suma
cuántos se ignoraron en el código. `squawk-ignore` vale, como en squawk, para su línea y la
siguiente (también al final de una línea o en un `/* */`, con el motivo después de otro `--`), y
`//nolint` de Go, con el alcance de golangci-lint. En `--format sarif` lo ignorado sale como
suprimido en el código (`inSource`), con su motivo, y GitHub lo muestra como descartado.

### Para agentes de IA

- `kerbo check --format agent`: salida corta con tope, las trampas primero y después lo que más
  se pasa del límite. Agrupa por archivo (la ruta va una vez), cada problema es una fila con
  columnas declaradas una sola vez (al estilo de TOON) y un `id` corto y estable, y cómo se
  arregla cada regla va una sola vez al final. Gasta entre 2,1 y 3,7 veces menos tokens que el
  JSON de antes. `--format agent-json` da lo mismo en JSON, para quien lo lee con código; el hook y
  el servidor MCP devuelven `--format agent`.
  Si el arreglo es mecánico (un import que sobra, un modificador de más, un `final` que falta),
  la sección `patch` trae, por `id`, el bloque de diff unificado que lo resuelve (en
  `agent-json`, el campo `patch` del problema), y la última línea, `autofix[N]: kerbo fix`, dice
  cuántos problemas del reporte entero arregla `kerbo fix` solo.
- `kerbo explain <regla>`: por qué existe, cómo se arregla y un ejemplo, corto para el contexto.
- **Hook de Claude Code**: revisa cada archivo que el agente edita, en milisegundos, y si encuentra
  algo se lo devuelve para que lo corrija en la misma vuelta. En `.claude/settings.json`:

  ```json
  {
    "hooks": {
      "PostToolUse": [
        { "matcher": "Edit|MultiEdit|Write", "hooks": [{ "type": "command", "command": "kerbo hook" }] }
      ]
    }
  }
  ```

  En un repo git sólo mira lo que cambió respecto de `HEAD`, antitrampa incluida, y respeta la línea
  de base: el agente no se pone a arreglar código que no tocó. Si kerbo falla, el hook no bloquea.
- **Demonio** (`kerbo daemon start`, `status` y `stop`): un proceso por usuario y máquina, con la
  caché en memoria, al que `kerbo check` y el hook le pasan el trabajo. Así varias sesiones de
  agentes en la misma máquina no arrancan todo de cero en cada edición. El hook lo arranca solo la
  primera vez. Atiende en paralelo los pedidos del mismo repo; los de otro repo esperan su turno.
  Escucha sólo en `127.0.0.1`, con un token que queda en la carpeta de caché, y se apaga solo
  después de 30 minutos sin uso. Si no está corriendo, o es de otra versión de kerbo, cada comando
  corre solo como siempre, y la salida es la misma. `KERBO_DAEMON=off` lo deja afuera.
- **Servidor MCP** (`kerbo mcp`, por stdio) con tres herramientas: `kerbo_check` (carpetas, `diff`
  y lenguajes; devuelve la salida para agentes), `kerbo_fix` (lo mismo que `kerbo fix`, con
  carpetas, reglas, lenguajes y `dry_run`; devuelve el parche y el resumen) y `kerbo_explain`. En Claude Code:
  `claude mcp add kerbo -- kerbo mcp`; en Cursor y otros clientes, un servidor stdio con el
  comando `kerbo` y el argumento `mcp`.

### En el editor

`kerbo lsp` es un servidor LSP: el editor le pasa el archivo mientras escribís y kerbo marca los
problemas al instante, sin esperar a que guardes, con el `kerbo.toml` y la línea de base del repo.
Los que tienen arreglo mecánico aparecen como acción rápida (*quick fix*): borrar el import,
sacar el modificador o agregar el `final`. Si el editor manda los problemas que tiene a la vista
(`context.diagnostics`), sólo se ofrecen los de esos; si no, los de las líneas pedidas. La acción
`source.fixAll.kerbo` arregla de una vez todo lo mecánico del archivo que no se pisa (en VS Code,
`"editor.codeActionsOnSave": { "source.fixAll": "explicit" }` lo corre al guardar).

- **IntelliJ IDEA, PyCharm, GoLand, WebStorm** (Community o Ultimate): instalá el plugin
  [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij), y en *Settings → Languages &
  Frameworks → Language Servers* agregá uno nuevo con el comando `kerbo lsp` y, en *Mappings →
  File name patterns*, `*.java`, `*.ts`, `*.tsx`, `*.js`, `*.py` y `*.go`.
- **Neovim**:

  ```lua
  vim.api.nvim_create_autocmd("FileType", {
    pattern = { "java", "typescript", "typescriptreact", "javascript", "python", "go" },
    callback = function() vim.lsp.start({ name = "kerbo", cmd = { "kerbo", "lsp" } }) end,
  })
  ```

- **Cualquier editor con LSP genérico** (Zed, Helix, Sublime con LSP): comando `kerbo lsp`, por
  stdio.

### `kerbo.toml`

Opcional, en la raíz del repo (kerbo lo busca hacia arriba desde la carpeta actual). Sin él vale
el preset `strict`. **Cada ajuste lleva su motivo**, y el resumen cuenta los desvíos del preset:
nada se afloja en silencio.

```toml
preset = "recommended"            # strict (de fábrica), recommended o minimal
lang = "es"                       # opcional: es o en; si falta, el del sistema

[[limits]]                        # un umbral para todos los lenguajes, o para uno
rule = "cyclomatic-complexity"
max = 12
language = "java"                 # opcional: typescript, python, java, go, rust, sql o docker
reason = "el equipo acordó 12 para los servicios"

[[exceptions]]                    # por carpeta o archivo
paths = ["src/legacy/**"]
rule = "cognitive-complexity"     # sin `max`, la regla no corre ahí; sin `rule`, ninguna
reason = "se reescribe en el primer trimestre"

[[ignore]]
paths = ["generated/**"]
reason = "lo genera el compilador de protos"

[cache]
max_megabytes = 512               # opcional: el tope de la caché, 256 de fábrica

[rust]                            # opcional: con qué se evalúan los #[cfg]
target_os = "windows"             # de fábrica, la máquina donde corre kerbo
target_arch = "aarch64"
features = ["std"]                # de fábrica, todas (como --all-features)
cfg = ["fast_path"]               # los que enciende el build.rs

[go]                              # opcional: qué archivos de Go entran, como go/build
goos = "linux"                    # de fábrica, GOOS o la máquina donde corre kerbo
goarch = "amd64"                  # de fábrica, GOARCH o la máquina
build_tags = ["integration"]      # como run.build-tags de golangci-lint
```

| Preset | Archivo | Función | Ciclomática | Cognitiva | Parámetros |
|---|---|---|---|---|---|
| `strict` | la tabla de abajo, por lenguaje | | | | |
| `recommended` | 750 | 150 | 15 | 20 | 6 |
| `minimal` | 1500 | 300 | 25 | 30 | 8 |

### GitHub Actions

```yaml
permissions:
  contents: read
  security-events: write          # para anotar los problemas en el pull request

jobs:
  kerbo:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0            # para --diff
      - uses: mati-caliz/kerbo@v0.2.0
        with:
          args: --diff origin/${{ github.base_ref }}
```

La acción instala el binario de Linux, sube el reporte SARIF (los problemas aparecen en la
pestaña de cambios del pull request) y falla el job si hay problemas.

## Reglas

| Regla | TypeScript / JavaScript | Python | Java | Go | Rust |
|---|---|---|---|---|---|
| `file-length` | 400 líneas de código | — | 500 líneas | 400 líneas | — |
| `function-length` | 120 líneas de código | — | 80 líneas desde la llave | 80 líneas internas | 80 líneas con código del cuerpo, contadas sobre el texto |
| `cyclomatic-complexity` | 10 | 10 | 8 | 10 | — |
| `cognitive-complexity` | 15 | — | 15 | 15 | 15, la de clippy y no la de Sonar |
| `parameter-count` | 4 | 6 (sin `self` ni `cls`) | 5 (sin `@Override`) | 4 | 4 (con el receptor, sin implementaciones de un trait) |
| `nesting-depth` | 4 bloques | — | — | 4 bloques de control | 4 bloques, `impl`, `trait` o módulos |
| `short-name` | menos de 2 caracteres salvo `i`, `j` y `_`, sin contar propiedades | `l`, `O` e `I` | locales de menos de 3 caracteres salvo `i` y `j` | menos de 3 caracteres si el último uso está a más de 5 líneas | — |
| `magic-number` | fuera de constantes de módulo, enums, tipos y defaults; -1, 0, 1 y 2 valen | sólo en comparaciones; 0 y 1 valen | fuera de campos, constantes y `hashCode`; -1, 0, 1 y 2 valen | en argumentos, asignaciones, casos, condiciones, operaciones y retornos; 0 y 1 valen | — |
| `duplicate-code` | funciones iguales de 3 líneas o más y ramas repetidas de un `if` o un `switch`, dentro del archivo | bloques de 100 tokens repetidos entre archivos o dentro de uno | lo mismo, sin contar anotaciones, `import` ni `;` | bloques de 150 nodos del árbol repetidos dentro de un paquete | — |
| `commented-code` | — | comentarios en su propia línea que compilan como Python | — | grupos de comentarios dentro de una función que compilan como una sentencia de Go | — |
| `unused-import` | — | — | imports que nadie nombra (los que sólo aparecen en el Javadoc cuentan como sin usar) y los de `java.lang`; se arregla solo | — | — |
| `redundant-import` | — | — | imports repetidos, de `java.lang` o del mismo paquete; se arregla solo | — | — |
| `star-import` | — | — | `import a.b.*`, también los `static` | — | — |
| `redundant-modifier` | — | — | lo que el lenguaje ya asume: `public`, `static`, `abstract` o `final` en una interfaz, `final` en un método privado o de una clase final, `static` en un enum o un record anidado, `public` en el constructor de una clase que no es pública; se arregla solo | — | — |
| `final-parameter` | — | — | parámetros de métodos y constructores con cuerpo que no son `final`; se arregla solo | — | — |
| `final-class` | — | — | clases con todos los constructores privados (o privadas y sin constructor) que no son `final` ni se heredan en el archivo; se arregla solo | — | — |

Cada lenguaje cuenta como su herramienta de referencia: ESLint y sonarjs, ruff, Checkstyle y PMD,
cyclop, gocognit, funlen, revive, mnd y varnamelen; los bloques duplicados, CPD de PMD, dupl y sonarjs (`no-identical-functions` y
`no-duplicated-branches`); el código comentado, ERA001 de ruff y `commentedOutCode` de gocritic; Rust, clippy. Esas diferencias de criterio no están documentadas en ningún
lado; cada una salió de comparar contra repos reales y quedó fijada en un test de
`src/metrics/tests.rs` (el anidamiento, los números mágicos y los nombres cortos, en los `tests.rs` de su módulo). En
los tests no se buscan números mágicos ni bloques duplicados, en los de TypeScript no se mide el largo de función y en
los de Python no se cuentan los parámetros. El preset `minimal` no busca números mágicos ni duplicados, y `recommended` marca
los bloques desde 150 tokens (225 nodos en Go y funciones de 5 líneas en TypeScript).

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
  `getParent`, `listFiles` y `list`. Esa parte anda sin classpath.
- `OptionalGetWithoutIsPresent` (`optional-get-without-is-present`): `get()` encadenado a
  `findFirst()`, `findAny()`, `max(...)`, `min(...)` u `Optional.ofNullable(...)`.

- `AutoCloseableResource` (`auto-closeable-resource`): una llamada que crea algo que se cierra
  (un `InputStream`, una `Connection`, un `ResultSet`, un `ExecutorService`, un `HttpClient`...)
  fuera de un `try` con recursos: en una variable que no se cierra, no se devuelve ni se pasa a
  otro método, o encadenada (`fabrica().getValidator()`). Qué se cierra sale de los jars, del tipo
  que declara un método del proyecto o de una tabla propia del JDK; lo que devuelve un `get...` es
  de quien lo da (el contenedor o el pool).

Y de seguridad, lo que llega del usuario sin limpiar a una consulta (`sql-injection`), a un
comando (`command-injection`), a una ruta de archivo (`path-traversal`) o a la respuesta HTML
(`xss`). Las fuentes son los
métodos de `HttpServletRequest` que devuelven lo que mandó el usuario (`getParameter`,
`getHeader`, `getCookies`, `getQueryString`...) y los parámetros con `@RequestParam`,
`@PathVariable`, `@RequestBody` y el resto de las anotaciones web de Spring. Los sumideros son
`Statement`, `Connection`, `JdbcTemplate`, `EntityManager` y `JdbcClient` (en una consulta sólo
cuenta el texto: los argumentos con `?` son seguros), `Runtime.exec` y `ProcessBuilder`, y
`File`, `FileInputStream`, `FileOutputStream`, `FileReader`, `FileWriter`, `RandomAccessFile`,
`Paths.get` y `Path.of`, y lo que se escribe con `response.getWriter()` o `getOutputStream()`
(encadenado o guardado en una variable). Un escape de HTML (`encodeForHTML` de ESAPI,
`HtmlUtils.htmlEscape`, `escapeHtml4`, `Encode.forHtml`...) protege la respuesta pero no una
consulta ni una ruta: el dato escapado se sigue aparte. Todo es tabla de datos en
`src/lint/java_taint/tables.rs`.

Cada método se recorre como un intérprete: sabe qué variables son constantes y decide con ellas los
`if`, los `switch` y los ternarios (`(7 * 42) - 86 > 200 ? "fijo" : dato` toma siempre la misma
rama), y sigue posición por posición las listas y clave por clave los mapas que arma (sacar
`lista.get(1)` cuando el dato entró en la 0 no es contaminarse). Entre métodos y entre archivos,
cada uno deja un resumen (qué devuelve según sus parámetros y qué llega a cada sumidero) y el
índice los cruza: una llamada va a los métodos de su tipo, a los que hereda y a sus
implementaciones; un tipo del mismo archivo tapa a los homónimos de otros. Lo que devuelve un
método abstracto de una interfaz que nadie implementa en el código (un repositorio de Spring Data)
sale de la base, no de sus argumentos. Una llamada a una librería devuelve lo que traen el receptor
y los argumentos, salvo las que dan un número, un booleano o un UUID (`parseInt`, `length`,
`equals`, `UUID.fromString`...).

En Python, las mismas cuatro reglas con el mismo motor. Las fuentes son `request.args`, `.form`,
`.cookies`, `.headers`, `.values`, `.files`, `.json`, `.get_json()` y compañía de Flask, Django
(`GET`, `POST`, `COOKIES`, `META`, `body`) y Pyramid (`params`, `matchdict`), los parámetros de una
función de ruta (`@app.route`, `@router.get`...) y el `event` de un handler de Lambda. Los
sumideros: `.execute`, `.executemany` y `.executescript`, `text()` y los métodos de un `Query` de
SQLAlchemy que reciben SQL en texto; `os.system`, `os.popen` y `os.exec*`/`spawn*` siempre, y
`subprocess.*` sólo con `shell=True` o si lo que corre es una shell con `-c` (sin shell, un texto
armado no se interpreta); `open`, `codecs.open`, `os.remove`, `os.path.exists`, `shutil.*`,
`send_file` y los métodos de un objeto de `pathlib`; y el cuerpo de la respuesta: lo que devuelve
una vista de Flask (no si devuelve un diccionario o usa `jsonify`; en FastAPI es JSON),
`make_response` y `render_template_string`. Un `if '..' in ruta: return` valida la ruta; un escape
de HTML (`html.escape`, `markupsafe.escape`, `render_template` o una función propia que se llame
"escape…html"), `shlex.quote` y `secure_filename` limpian para su sumidero; lo que arma una
sentencia de SQLAlchemy (`select`, `insert(...).values(...)`) va con parámetros ligados. Entre
módulos, las llamadas se resuelven por los imports (también los relativos y los de adentro de una
función), las funciones y clases que se ven desde cada una, `self` y la clase con que se creó una
variable. `os.name` se toma como `"posix"`: es donde corre el servidor.

En JavaScript y TypeScript, las mismas cuatro reglas. Sin tipos, las fuentes y los sumideros se
reconocen por el nombre: el pedido (`req`, `request`, `ctx`) y sus `query`, `params`, `body`,
`cookies`, `headers`, `url` y compañía; el `event` de un handler de Lambda (`(event, context)`, o
un único `event` en una función que se llama `handler`); y en el navegador, `location.hash`,
`location.search`, `document.URL`, `document.referrer` y `window.name`. Los sumideros: `query`,
`execute`, `raw` y `$queryRawUnsafe` sobre una conexión (`db`, `conn`, `pool`, `client`, `knex`,
`prisma`...); `child_process.exec` siempre y `spawn`/`execFile` con `{ shell: true }` o con el programa elegido por el usuario;
`fs.*` y `res.sendFile`; y para HTML, `res.send`, `res.write` y `res.end` de Express y Node (no
los de Fastify, que manda texto plano, ni si la función declara otro tipo con `res.type('json')`
o `writeHead(200, { 'content-type': 'application/json' })`), `innerHTML`, `document.write`,
`insertAdjacentHTML`, `dangerouslySetInnerHTML`, jQuery (`$("<b>" + x)`, `.html(x)`) y la
respuesta de un handler de Lambda que se declara `text/html`. `DOMPurify.sanitize`,
`sanitize-html`, `escape-html`, `$sanitize` o una función propia que se llame "escape…html"
limpian el HTML; `path.basename`, la ruta. Cada función (también las anónimas de una ruta) es un
método del motor, y entre archivos se resuelven `import` y `require`, también los relativos.

En Go, las mismas cuatro reglas, y como Go declara los tipos, las fuentes y los sumideros salen de
ellos: un parámetro `*http.Request` o un evento de Lambda (`events.APIGatewayProxyRequest`) trae
todo del usuario, y de un `*gin.Context` o un `echo.Context` sólo lo que lee el pedido (`Query`,
`Param`, `PostForm`...). Los sumideros: `Query`, `QueryRow`, `Exec` y `Prepare` (y sus
`...Context`, con la consulta después del `ctx`) de `*sql.DB`, `sqlx` y `pgx`, y `Raw`, `Where`,
`Order` y compañía de gorm, siguiendo el tipo por la cadena (`db.Table("x").Order(campo)`);
`exec.Command` con una shell y `-c`; `os.Open`, `os.ReadFile`, `os.Remove` y compañía y
`http.ServeFile`; y `w.Write`, `fmt.Fprintf(w, ...)`, `io.WriteString(w, ...)` sobre un
`http.ResponseWriter` (no si la función pone un `Content-Type` que no es HTML), `template.HTML` y
la respuesta de Lambda que se declara `text/html`. `html.EscapeString`, `template.HTMLEscapeString`
y `json.Marshal` (escapa `<` y `>`) limpian el HTML; `filepath.Base` y `path.Clean("/" + ruta)`, la
ruta; `strconv.Atoi` y compañía dan un número. Lo escrito en un `strings.Builder` queda en él. Las
funciones anónimas (el `func(w, r)` de un `HandleFunc`) ven los tipos de las variables de las que
las contienen, y entre paquetes las llamadas se resuelven por la carpeta: la ruta del import
termina como la carpeta del paquete.

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

En Rust, kerbo mide sólo lo que mide clippy (`too_many_lines`, `too_many_arguments`,
`cognitive_complexity` y `excessive_nesting`), y la complejidad cognitiva es la de clippy, no la de
Sonar: arranca en 1 y suma cada `if`, cada `match` de más de un brazo y cada guarda sobre el código
ya expandido, así que también suman los `for`, los `while`, cada `?` y cada `.await`, los `assert!`
y los `matches!`. Después descuenta los `return`: todos si la función devuelve `Result`, la mitad
si no. Los closures se miden aparte, también los que van adentro de una macro. Las `macro_rules!`
del propio crate se expanden como rustc, leyendo las definiciones de todos sus archivos, y también
las exportadas por las dependencias (`dep::macro!`, `use dep::macro` o `#[macro_use] extern
crate`), leídas de la caché de cargo con la versión del `Cargo.lock` o, sin lockfile, la más nueva
compatible con lo pedido. Las de la biblioteca estándar se imitan; de las macros procedurales se
cuentan sólo los argumentos.

Como clippy mide lo que compila, un ítem, una sentencia o un brazo de `match` con un `#[cfg]` falso
no cuenta para la complejidad ni para el anidamiento, y un archivo con `#![cfg(..)]` falso no se
mide. Los `#[cfg]` se evalúan con la máquina donde corre kerbo, todas las features del
`Cargo.toml` y `test` en los dos modos (la biblioteca y sus tests); lo que kerbo no conoce, como
un `cfg` que emite el `build.rs`, no decide y el código se mide. La sección `[rust]` de
`kerbo.toml` cambia la plataforma, las features y los `cfg` encendidos.

### Python contra ruff

En Python, kerbo hace las reglas del conjunto clásico de ruff (`E4`, `E7`, `E9` y `F`: pyflakes y
los errores de pycodestyle) con el mismo código que ruff, en minúscula: `kerbo explain f401`,
`--max f841=0` o `f401 = "off"` en `kerbo.toml`. Respeta `# noqa`, `# noqa: F401` y
`# ruff: noqa` como ruff (en strings de varias líneas y líneas continuadas con `\`, el `noqa` va
en la última). `minimal` deja sólo las `F`.

| Reglas | Qué marca |
|---|---|
| `f401` | imports sin usar (se arregla solo, salvo en `__init__.py`) |
| `f811`, `f402` | redefiniciones de algo nunca usado y variables de un `for` que pisan un import |
| `f821`, `f405`, `f822`, `f823` | nombres no definidos (con los alcances de módulo, clase, función y comprensión, las anotaciones entre comillas y `from __future__ import annotations`), los que pueden venir de un `import *`, los de `__all__` y los locales usados antes de asignarse |
| `f841`, `f842` | variables locales asignadas o anotadas y nunca usadas (el `as` de un `except` se arregla solo) |
| `f403`, `f404`, `f406` | `import *`, `from __future__` tarde y `import *` fuera del módulo |
| `f501`–`f509`, `f521`–`f525` | formatos con `%` y `.format()` que no cierran con sus argumentos |
| `f541` | f-strings sin reemplazos (se arregla solo) |
| `f601`, `f602` | claves repetidas en un diccionario |
| `f631`, `f632`, `f633`, `f634` | `assert` e `if` sobre una tupla, `is` contra un literal (se arregla solo) y `print >>` |
| `f701`, `f702`, `f704`, `f706`, `f707`, `f722`, `f901` | `break`, `continue`, `yield`, `await` y `return` fuera de lugar, `except:` antes de otros, anotaciones entre comillas que no compilan y `raise NotImplemented` (se arregla solo) |
| `e401`, `e402` | varios módulos en un `import` e imports después de otro código |
| `e701`, `e702`, `e703` | varias sentencias en una línea (el `;` final se arregla solo) |
| `e711`, `e712`, `e713`, `e714`, `e721` | `== None`, `== True`, `not x in y`, `not x is y` (se arreglan solos) y comparaciones de tipos con `==` |
| `e722`, `e731`, `e741`, `e742`, `e743` | `except:` pelado, lambdas asignadas y nombres `l`, `I` y `O` |

Paridad, contra ruff 0.17 con `--select E4,E7,E9,F`: cero diferencias por archivo, línea, columna y
regla en django, fastapi, requests, flask, pydantic, sqlalchemy, ipython, ansible, sympy y la
biblioteca estándar de CPython (unos 11.000 archivos y 15.900 avisos). Las únicas que quedan son
las de archivos que tree-sitter-python no parsea (ahí kerbo no corre las reglas de alcance) y E902,
que es un error de lectura y no una regla.

### TypeScript y JavaScript contra ESLint

En TypeScript y JavaScript, kerbo hace las reglas más frecuentes de `@eslint/js` recommended y de
`typescript-eslint` recommended que no piden tipos, con el nombre de ESLint sin el prefijo del
plugin: `kerbo explain no-unused-vars`, `--max no-explicit-any=0` o `no-var = "off"`. Corren en los
dos lenguajes como en un `eslint.config.js` con `js.configs.recommended` y
`tseslint.configs.recommended`; en los `.ts` no corre `no-dupe-keys`, que ya revisa el compilador.
`no-var` y `prefer-const` van también en JavaScript y `no-non-null-assertion` (de `strict`) entra
en el preset `strict`. `recommended` la apaga y `minimal` deja sólo las que ESLint clasifica como
problemas. Los `eslint-disable` no silencian nada: el aviso sale y el comentario lo marca la
antitrampa.

| Reglas | Qué marca |
|---|---|
| `no-unused-vars` | variables, funciones, clases, tipos, imports y parámetros sin usar, con los alcances de `@typescript-eslint/scope-manager` (tipos y valores por separado, `typeof`, JSX contra `React`, declaraciones ambientales y `.d.ts`) |
| `prefer-const`, `no-var` | `let` que nunca se reasigna y `var`; los dos se arreglan solos cuando ESLint los arregla |
| `no-unused-expressions` | expresiones sueltas que no hacen nada |
| `no-require-imports` | `require()` sin un `require` propio e `import x = require()` |
| `no-explicit-any`, `no-non-null-assertion`, `ban-ts-comment` | `any`, el `!` de no nulo y `@ts-ignore`, `@ts-nocheck` o `@ts-expect-error` sin motivo |
| `no-useless-escape` | escapes que no hacen falta en strings, templates y expresiones regulares (se arregla solo) |
| `prefer-rest-params`, `no-prototype-builtins`, `no-this-alias`, `no-empty-object-type` | `arguments`, `obj.hasOwnProperty()`, `const self = this` y el tipo `{}` |
| `no-empty`, `no-cond-assign`, `no-case-declarations` | bloques vacíos, asignaciones en condiciones y declaraciones sueltas en un `case` |
| `no-debugger`, `no-duplicate-case`, `no-dupe-else-if`, `no-dupe-keys` | `debugger` (se arregla solo), casos, ramas y claves repetidas |

Paridad, contra ESLint 10.13 y typescript-eslint 8.72: ninguna diferencia de más por archivo,
línea, columna y regla en next.js, react, VS Code, zod y express (42.488 avisos en unos 30.000
archivos). Faltan 587 avisos, todos de archivos que tree-sitter no entiende (varianza `in`/`out`,
`let as`, `export type *`, escapes en tipos de template) o de `.js` con argumentos de tipo en
llamadas, que TypeScript lee como comparaciones.

### Shell contra ShellCheck

Los `.sh`, los `.bash` y los archivos sin extensión con shebang de `sh`, `bash`, `dash` o busybox
se revisan contra [ShellCheck](https://www.shellcheck.net/), con su código como nombre y en
minúscula: `kerbo explain SC2086`, `--max sc2086=0` o `sc2034 = "off"` en `kerbo.toml`. Son las 22
reglas que más aparecen, las 64 de lo que bash agrega en un script de sh (`sc3001` a `sc3067`) y 55
chequeos de comandos (`tr`, `grep`, `expr`, `find`, `rm`, `sudo`, `return`, `alias`, `read` y otros); la severidad es la de ShellCheck, `recommended` apaga las de estilo y
`minimal`, también las informativas. Las reglas universales de la tabla de arriba no corren en
shell, salvo la antitrampa.

| Reglas | Qué marca |
|---|---|
| `sc2086` | expansiones sin comillas que pueden partirse o expandir comodines, con el valor de cada variable seguido por ramas, loops y funciones como ShellCheck (se arregla solo) |
| `sc2046`, `sc2206`, `sc2207`, `sc2068` | `$(…)` y `${arr[@]}` sin comillas, y arreglos armados con palabras que se parten |
| `sc2006`, `sc2005`, `sc2116` | backticks (se arregla solo), `echo $(cmd)` y `$(echo …)` |
| `sc2034` | variables asignadas y nunca usadas |
| `sc2155` | `local x=$(cmd)`, que esconde el código de salida |
| `sc2162`, `sc2164` | `read` sin `-r` y `cd` sin `\|\| exit` (los dos se arreglan solos) |
| `sc2004`, `sc2181`, `sc2015`, `sc2166` | `$` de más en aritmética, `[ $? -ne 0 ]`, `a && b \|\| c` y `-a`/`-o` en `[ ]` |
| `sc2016`, `sc2059`, `sc2064`, `sc2069`, `sc2223` | expansiones entre comillas simples, variables en el formato de `printf`, `trap` con comillas dobles, redirecciones al revés y `: ${x:=…}` sin comillas |
| `sc2148` | un script sin shebang ni `# shellcheck shell=` |
| `sc2003`, `sc2018` a `sc2029`, `sc2060` a `sc2063`, `sc2114` a `sc2313` | lo de `Checks/Commands.hs` de ShellCheck: `tr a-z`, globs sin comillas en `find -name`, `grep` y `unset`, `expr`, `rm -rf "$dir/"`, `sudo` con redirecciones, `return`/`exit` fuera de 0-255, `export $x`, `alias` con `$1`, `egrep`, `mv` sin destino y el resto |
| `sc3001` a `sc3067` | en un script de `sh`, `dash` o busybox, lo que sólo tiene bash: `[[ ]]`, `local`, arreglos, `${x/a/b}`, `echo -n`, `&>`, `source`, `set -o pipefail` y el resto de `checkBashisms` de ShellCheck, con las excepciones de dash y busybox |

`# shellcheck disable=` vale como en ShellCheck: antes del primer comando, para el archivo entero;
antes de un comando compuesto, para todo el bloque; acepta rangos (`SC2000-SC2099`) y `all`.

Paridad, contra ShellCheck 0.11: cero diferencias por archivo, línea, columna y regla en 1.752
scripts de kubernetes, moby, nvm, rustup, ohmyzsh, acme.sh, bash-it, dokku y otros (1.680 avisos).
Con esos repos actualizados (2026-10-30, 1.834 scripts): 1.884 avisos idénticos y 87 de más, todos
`sc2016` de un `nvm.sh` nuevo que tree-sitter-bash lee mal; sin las directivas `# shellcheck`,
3.460 idénticos (96 de `sc3*` y 51 de los chequeos de comandos) y ninguno de más. Los archivos que tree-sitter-bash no parsea se
saltean.

### Go contra golangci-lint

En Go, kerbo hace reglas del conjunto por defecto de golangci-lint con el mismo nombre:
`kerbo explain errcheck`, `--max ineffassign=0` o `qf1003 = "off"` en `kerbo.toml`. `errcheck`
marca el `error` que devuelve una llamada y nadie revisa, con las exclusiones de errcheck; los
tipos los saca el chequeo de tipos del propio Go con un helper que kerbo compila una vez con el
`go` del sistema (sin `go`, el escaneo falla con el motivo, o se apaga con `errcheck = "off"`). `ineffassign` marca una asignación que nadie lee antes de que se
pise o termine la función, siguiendo ramas, loops, `switch` con `fallthrough`, `select`, `goto` y
etiquetas; no cuentan las variables que captura un closure ni aquellas cuya dirección se toma. De
staticcheck: `st1005`, mensajes de error con mayúscula o puntuación al final (con las constantes de
todo el paquete y las mismas excepciones), `qf1003`, cadenas de `if` que comparan siempre lo mismo
y podrían ser un `switch`, y `qf1004`, `strings.Replace` y compañía con `-1` de límite.

Como golangci-lint, saltea los archivos generados, los que no compilan en la plataforma (sufijos
como `_windows.go`, `//go:build` y `// +build`) y lo que `./...` no lista desde el `go.mod`
(`testdata`, `vendor` y las carpetas que empiezan con `_` o `.`). La plataforma es la de `GOOS` y
`GOARCH` o la máquina donde corre kerbo; la sección `[go]` de `kerbo.toml` la fija y suma build
tags.

`//nolint` vale como en golangci-lint: al final de una línea, para esa línea; en su propia línea,
para lo que empieza justo abajo en la misma columna (una función, un `if`, el archivo entero si
está antes de `package`). Sin lista o con `all` silencia todas las reglas, y el motivo de
`//nolint:ineffassign // motivo` aparece en el reporte.

Paridad, contra golangci-lint 2.15 en linux/amd64: cero diferencias por archivo, línea y columna
en golang/tools, terraform, hugo, caddy, consul, prometheus, etcd, syncthing y otros (1.381
avisos), en errcheck 10.754, en `qf1008` y `qf1012` 1.153 y en `sa1019` (uso de lo deprecado)
592, `qf1001` 136, `stdmethods` de govet 28 y la cola chica de staticcheck (`st1012`, `qf1011`,
`st1023`, `qf1002`, `s1005`, `sa9003`, `qf1006`, `st1019`, `sa1029`, `sa9004`, `qf1009`, `st1013`,
`s1000`, `st1017`, `st1006`, `s1008`, `s1002` y `s1034`) 204 y en `unused` (código que nadie usa) 249, sin
ninguno de más: el 99 % de lo que marca golangci-lint por defecto. Suma otras 42 reglas chicas de
staticcheck (`s1009`, `s1011`, `s1017`, `s1038`, `sa1006`, `sa2001`, `sa4004`, `sa4016`, `sa6002`,
`st1008`, `qf1007` y más) que dan lo mismo que golangci-lint en sus casos de prueba y en los 26 avisos del corpus. Lo demás de
govet y de staticcheck sigue en golangci-lint.

### Secretos y dependencias vulnerables

`committed-secret` revisa todos los archivos versionados, no sólo el código, con las 222 reglas de
la configuración por defecto de gitleaks (claves de AWS, GCP y Azure, tokens de GitHub, GitLab,
Slack y Stripe, claves privadas, JWT…), sus allowlists, su umbral de entropía y su decodificación
de base64, hex, URL y escapes Unicode, más una regla propia para contraseñas en cadenas de
conexión. Marca lo mismo que `gitleaks dir` en los repos donde se midió. El secreto nunca sale
entero en el reporte ni se guarda en la caché: sólo los primeros cuatro caracteres y el largo. Un
`gitleaks:allow` en la línea lo deja pasar; los datos de prueba se excluyen en `kerbo.toml` con
`[[exceptions]]`, `rule = "committed-secret"` y su `reason` (los `.gitleaksignore` no se leen,
para que nada se afloje en silencio).

`vulnerable-dependency` es la única regla que usa la red, y sólo con `--osv`: lee `Cargo.lock`,
`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `go.mod`, `go.sum`, `poetry.lock`, `uv.lock`,
`Pipfile.lock`, `requirements*.txt` fijados con `==`, `pom.xml` (con los padres locales y sus
propiedades) y `gradle.lockfile`, y le pregunta a OSV por las versiones exactas. Habla con
`api.osv.dev` a través de `curl` (el que trae el sistema), con un minuto de tope por pedido, y
guarda las respuestas en la caché un día; los avisos se guardan hasta que OSV los modifique.
`kerbo osv-download` baja la base de cada ecosistema (unos 280 MB en total, npm es casi todo) y
`--osv=offline` la usa sin red. Cada problema dice el aviso, sus alias y la versión que lo arregla.
Las dos reglas están en los tres presets, también en los tests.

Esos son los umbrales del preset `strict`. Más reglas y la capa para agentes de IA están en la
[hoja de ruta](ROADMAP.md).

## Migraciones y contenedores

kerbo también revisa lo que se despliega junto al código: las migraciones de Postgres, los
Dockerfile y los archivos de docker compose.

**Migraciones.** Son migraciones los `.sql` dentro de una carpeta `migrations`, `migration` o
`drizzle` (Prisma, drizzle, sqlx, golang-migrate) y los que siguen el nombre de Flyway
(`V3__agrega_indice.sql`, `R__vistas.sql`); el resto de los `.sql` (semillas, dumps) no se mira.
En Alembic se lee el `upgrade()` de cada revisión: cada `op.*` y `batch_op.*` se convierte en el
SQL que Alembic generaría para Postgres (tipos de SQLAlchemy, `server_default`, claves foráneas, la
clave primaria entera como `SERIAL`), junto con el de `op.execute(...)`, y todo pasa por las mismas
reglas que un `.sql`, con el aviso en la línea de la llamada. Se siguen los `for` sobre listas, las
funciones auxiliares del mismo archivo y las constantes del módulo, `autocommit_block()` sale de la
transacción, y las ramas de otro dialecto (`if dialect == "mysql"`) y los `except` se saltean. Las reglas son las de
[squawk](https://squawk.sh), con sus nombres y su criterio, suponiendo como él con
`--assume-in-transaction` que cada archivo corre en una transacción (así lo hacen Prisma,
drizzle, Flyway y Alembic en Postgres): un índice sobre una tabla creada en la misma migración no
se marca.

| Regla | Qué marca |
|---|---|
| `require-concurrent-index-creation` | `CREATE INDEX` sin `CONCURRENTLY` sobre una tabla que ya existía |
| `require-concurrent-index-deletion` | `DROP INDEX` sin `CONCURRENTLY` |
| `adding-required-field` | `ADD COLUMN ... NOT NULL` sin `DEFAULT` ni columna generada |
| `adding-not-nullable-field` | `SET NOT NULL` sin un `CHECK (columna IS NOT NULL) NOT VALID` validado antes |
| `changing-column-type` | `ALTER COLUMN ... TYPE` (también en tipos compuestos) |
| `renaming-column`, `renaming-table` | renombres de columnas, atributos y tablas |
| `ban-drop-column`, `ban-drop-table` | borrados de columnas, atributos y tablas |
| `require-lock-timeout`, `require-statement-timeout` | la primera operación que toma locks sin un `SET lock_timeout` o `statement_timeout` antes |
| `constraint-missing-not-valid`, `adding-foreign-key-constraint` | restricciones y claves foráneas sin `NOT VALID`, o validadas en la misma transacción |
| `adding-serial-primary-key-field`, `disallowed-unique-constraint` | `PRIMARY KEY` y `UNIQUE` sin `USING INDEX` sobre una tabla que ya existía |
| `adding-field-with-default` | una columna con un default volátil (`random()`, `gen_random_uuid()`) o generada |
| `prefer-bigint-over-int`, `prefer-bigint-over-smallint`, `prefer-identity`, `prefer-text-field`, `prefer-timestamp-tz`, `ban-char-field` | `int`, `smallint`, `serial`, `varchar(n)`, `timestamp` sin zona y `char(n)` en columnas nuevas o cambiadas |
| `ban-drop-constraint`, `ban-drop-default`, `ban-drop-not-null`, `ban-drop-generated-expression` | borrados de restricciones, defaults, `NOT NULL` y expresiones generadas |
| `ban-drop-type`, `ban-drop-function`, `ban-drop-sequence`, `ban-drop-view`, `ban-drop-schema`, `ban-drop-domain`, `ban-drop-extension`, `ban-drop-database` | borrados de objetos que el código puede usar |
| `renaming-object`, `ban-set-schema` | renombres de restricciones, índices, tipos, vistas y funciones, y objetos movidos a otro esquema |
| `require-enum-value-ordering`, `ban-alter-identity`, `ban-create-domain-with-constraint`, `ban-alter-domain-with-add-constraint` | valores de enum sin posición, identidades cambiadas y dominios con restricciones |
| `require-concurrent-reindex`, `require-concurrent-partition-detach`, `ban-truncate-cascade` | `REINDEX` y `DETACH PARTITION` sin `CONCURRENTLY`, y `TRUNCATE ... CASCADE` |
| `transaction-nesting`, `ban-uncommitted-transaction`, `ban-concurrent-index-creation-in-transaction`, `prefer-robust-stmts` | `BEGIN`/`COMMIT` dentro de la transacción de la herramienta, transacciones sin cerrar, `CONCURRENTLY` en una transacción y sentencias sin `IF [NOT] EXISTS` fuera de ella |
| `identifier-too-long`, `ban-duplicate-column-assignments` | nombres de más de 63 bytes y columnas repetidas en una tabla, un `INSERT`, un `SET` o una vista |

Es el conjunto por defecto de squawk 2.70 entero, salvo `prefer-repack`, que squawk sólo corre con
Postgres 19. `recommended` apaga las de preferencia de tipos (`prefer-*` y `ban-char-field`) y
`minimal`, también los timeouts. Paridad, contra squawk 2.70 con `--assume-in-transaction`: los
9.612 avisos de squawk, ninguno de más, en 1.858 migraciones de lobe-chat (drizzle), documenso,
trigger.dev y cal.com (Prisma), conductor (Flyway) y awesome-compose. En Alembic, contra squawk sobre
el SQL que genera el propio Alembic en modo offline (300 revisiones de airflow, superset, mlflow y
redash; las otras 349 importan la aplicación o consultan la base y no se pueden convertir sin
levantarla): 297 idénticas por regla y cantidad, ninguna con avisos de menos, y dos con avisos de
más en caminos que el modo offline no recorre (`is_offline_mode()` y un `try` que en offline se
corta).

**Dockerfile.** `Dockerfile`, `Containerfile`, `Dockerfile.*` y `*.Dockerfile`, contra
[hadolint](https://github.com/hadolint/hadolint): todas las reglas DL que hadolint trae prendidas
(59), con su código como nombre (`kerbo explain DL3008`). El shell de cada `RUN` se lee con un
parser propio que separa los comandos como ShellCheck. Quedan afuera las reglas de ShellCheck
(`SC*`), que van con la etapa 10, y las que hadolint trae apagadas o sólo corren con su
configuración (DL1001, DL3026, DL3049 a DL3058). `.hadolint.yaml` no se lee: una regla se apaga en
`kerbo.toml`, con su motivo. `recommended` apaga las de nivel `info` de hadolint y `minimal` deja
sólo las de nivel `error`. Paridad, contra hadolint 2.15.1 con su configuración por defecto: los
408 avisos de hadolint, ninguno de más, en 169 Dockerfiles de awesome-compose, docker-stacks,
airflow, superset, mlflow, nextcloud/docker, mastodon, sentry self-hosted, grafana y redash.

**docker compose.** `compose.yaml`, `docker-compose.yml` y sus variantes (`docker-compose.prod.yml`),
con anclas y `<<:` resueltos. No hay herramienta de referencia: el criterio es propio.

| Regla | Qué marca |
|---|---|
| `compose-memory-limit` | un servicio con `image` o `build` sin `deploy.resources.limits.memory` ni `mem_limit`; los que usan `extends` y los fragmentos de un override se saltean |
| `compose-healthcheck-start-period` | un `healthcheck` activo sin `start_period` |
| `compose-healthcheck-interval` | un `interval` de menos de 5 s o más de 5 min, o un `timeout` igual o mayor que el intervalo |
| `compose-cpu-limit` | un servicio con `image` o `build` sin `deploy.resources.limits.cpus` ni `cpus` |
| `compose-restart` | un servicio sin `restart` ni `deploy.restart_policy` (`restart: "no"` vale para una tarea que corre una vez) |
| `compose-image-tag` | una imagen sin tag o con `latest`; un digest o una variable no cuentan |
| `compose-depends-on-healthy` | un `depends_on` hacia un servicio con healthcheck que no pide `condition: service_healthy` |

Cada una se apaga como cualquier otra regla, con `[[limits]]` sin `max` y su motivo; el `max` no
aplica. Con `--language sql` o `--language docker` se analiza sólo eso.

## Cómo está armado

Las métricas se escriben una sola vez sobre el árbol de [tree-sitter](https://tree-sitter.github.io/).
Lo propio de cada lenguaje vive en `src/profile/`: qué nodo es una función, una decisión o un
operador lógico, y cómo cuenta líneas la herramienta que se imita. Los archivos se analizan en
paralelo y de a uno, sin cargar el programa entero.

La caché (`src/cache/`) guarda por archivo lo que mide el análisis, no los problemas: la clave es el
contenido, el lenguaje, los umbrales efectivos de ese archivo y la identidad del binario (versión,
tamaño y fecha), y la ruta queda afuera, así un archivo movido o el mismo archivo en otro clone no
se vuelve a analizar. Las reglas y los duplicados entre archivos se calculan siempre sobre lo
guardado. Está partida en 256 archivos por el primer byte de la clave, cada uno con su tope y
desalojo del menos usado; cada entrada va comprimida con LZ4 y con su suma de control, y se
escribe a un temporal que se renombra. En Rust el resultado de un archivo depende también de las
`macro_rules!`, así que la clave suma una huella de los archivos del crate que definen macros, las
traen con `#[macro_use]` o invocan una que arma funciones, de la versión de cada dependencia y de
la ruta del archivo: cambiar una macro vuelve a analizar a quien la usa, y editar una función
común, no.

Los textos del CLI (reporte, `explain`, errores, hook, MCP y LSP) viven en un catálogo tipado en
`src/i18n/`, uno por idioma; el compilador asegura que cada mensaje exista en los dos.

Un arreglo mecánico (`src/fix.rs`) es una lista de ediciones de rango (línea y columna de inicio y
de fin y el texto nuevo) más el bloque de diff unificado que resulta, calculado durante el análisis
y guardado en la caché con el resto. El LSP manda las ediciones como acción rápida y `--format
agent` manda el diff; una regla nueva sólo tiene que armar el `Fix` (borrar un nodo o insertar
antes de uno) y le llegan solos `kerbo fix`, `kerbo_fix` y el "arreglar todo" del LSP, que eligen
los arreglos que no se pisan con `fix::compatible`.

Los comentarios que silencian una herramienta original están en un solo registro,
`SYNTAXES` de `src/suppressions.rs`: el comentario (`--` o `#`), el marcador y si vale para la
línea siguiente o para el archivo. De ahí salen tanto la antitrampa como lo que se ignora al
reportar; los códigos se comparan con el nombre de la regla sin distinguir mayúsculas, así que una
herramienta nueva sólo suma su fila.

## Desarrollo

El gate (formato, clippy en modo estricto, techos de tamaño, tests con cobertura y `cargo-deny`)
es `scripts/verify-rust.sh` y corre en un contenedor, así que sólo hace falta Docker. En Windows,
desde el bash de Git, va con `MSYS_NO_PATHCONV=1 bash scripts/verify-rust.sh`: sin eso Git Bash
convierte las rutas que se le pasan a `docker run`. El `.gitattributes` deja los archivos de texto
con LF aunque `core.autocrlf` esté prendido, así `cargo fmt --check` no falla por los finales de
línea.

## Licencia

[Apache 2.0](LICENSE).
