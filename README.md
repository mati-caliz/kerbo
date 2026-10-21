# kerbo

Español · [English](README.en.md)

Analizador de calidad de código para **Java, TypeScript/JavaScript, Python, Go y Rust** en un solo
binario. Aplica las mismas reglas en todos los lenguajes, en segundos y con pocos megas de
memoria, y cuenta igual que la herramienta que ya usás en cada uno (ESLint, ruff, Checkstyle y
PMD, golangci-lint, clippy). También revisa las migraciones de Postgres como squawk, los
Dockerfile como hadolint y los archivos de docker compose.

```console
$ kerbo check
src/billing/InvoiceService.java:88:5: cyclomatic-complexity La función `settle` tiene complejidad ciclomática 11 (máximo 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity La función `applyPromotions` tiene complejidad cognitiva 19 (máximo 15)
kerbo: 5810 archivos, 2 problemas, 2250 ms
```

## Instalación

Linux y macOS, en x86_64 y ARM (en Linux, un binario estático que corre en cualquier distro), y
Windows x86_64 desde el bash de Git:

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

El instalador baja la última versión de los
[releases](https://github.com/mati-caliz/kerbo/releases) para la máquina, verifica el sha256 y
deja `kerbo` en `~/.local/bin` (otra carpeta con `KERBO_INSTALL_DIR`).

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
archivo. Dos corridas a la vez no se pisan: antes de escribir, cada una relee lo que dejó la otra.
Si se rompe, se ignora.

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
contexto (`git apply --unidiff-zero`). Sale con `0` si todo quedó escrito, `1` si algún archivo no
se escribió porque el arreglo dejaba algo nuevo y `2` si algo no se pudo leer o escribir.

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
cuántos se ignoraron en el código.

### Para agentes de IA

- `kerbo check --format agent`: salida corta con tope, las trampas primero y después lo que más
  se pasa del límite. Agrupa por archivo (la ruta va una vez), cada problema es una fila con
  columnas declaradas una sola vez (al estilo de TOON) y un `id` corto y estable, y cómo se
  arregla cada regla va una sola vez al final. Gasta entre 2,1 y 3,7 veces menos tokens que el
  JSON de antes. `--format agent-json` da lo mismo en JSON, para quien lo lee con código; el hook y
  el servidor MCP devuelven `--format agent`.
  Si el arreglo es mecánico (un import que sobra, un modificador de más, un `final` que falta),
  la sección `patch` trae, por `id`, el bloque de diff unificado que lo resuelve (en
  `agent-json`, el campo `patch` del problema).
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

En Rust, kerbo mide sólo lo que mide clippy (`too_many_lines`, `too_many_arguments`,
`cognitive_complexity` y `excessive_nesting`), y la complejidad cognitiva es la de clippy, no la de
Sonar: arranca en 1 y suma cada `if`, cada `match` de más de un brazo y cada guarda sobre el código
ya expandido, así que también suman los `for`, los `while`, cada `?` y cada `.await`, los `assert!`
y los `matches!`. Después descuenta los `return`: todos si la función devuelve `Result`, la mitad
si no. Los closures se miden aparte, también los que van adentro de una macro. Las `macro_rules!`
del propio crate se expanden como rustc, leyendo las definiciones de todos sus archivos; las de la
biblioteca estándar y las de `log` se imitan, y de las de otras dependencias se cuentan sólo los
argumentos.

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
En Alembic se lee el `upgrade()` de cada revisión: las llamadas `op.*` y `batch_op.*` se traducen
a la regla que marcaría el SQL que generan, el SQL de `op.execute(...)` se analiza como cualquier
migración y las ramas de otro dialecto (`if dialect == "mysql"`) se saltean. Las reglas son las de
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

**Dockerfile.** `Dockerfile`, `Containerfile`, `Dockerfile.*` y `*.Dockerfile`, contra
[hadolint](https://github.com/hadolint/hadolint): las 38 reglas DL más comunes, con su código como
nombre (`kerbo explain DL3008`). El shell de cada `RUN` se lee con un parser propio que separa los
comandos como ShellCheck. Quedan afuera las reglas de ShellCheck (`SC*`), las que hadolint trae
apagadas o atadas a su configuración (DL1001, DL3026 y DL3048 a DL3058) y DL3001, DL3010, DL3011,
DL3028, DL3030, DL3033 a DL3041, DL3043, DL3044, DL3046, DL3060, DL3063, DL3065 y DL4005. `recommended` apaga las de nivel `info` de hadolint y `minimal` deja sólo las de nivel
`error`.

**docker compose.** `compose.yaml`, `docker-compose.yml` y sus variantes (`docker-compose.prod.yml`),
con anclas y `<<:` resueltos. No hay herramienta de referencia: el criterio es propio.

| Regla | Qué marca |
|---|---|
| `compose-memory-limit` | un servicio con `image` o `build` sin `deploy.resources.limits.memory` ni `mem_limit`; los que usan `extends` y los fragmentos de un override se saltean |
| `compose-healthcheck-start-period` | un `healthcheck` activo sin `start_period` |
| `compose-healthcheck-interval` | un `interval` de menos de 5 s o más de 5 min, o un `timeout` igual o mayor que el intervalo |

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
`macro_rules!` del crate, así que la clave suma una huella del texto de todos los archivos del
crate y de la ruta del archivo dentro de él: cambiar una macro vuelve a analizar a quien la usa.

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
