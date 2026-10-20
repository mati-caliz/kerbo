# Hoja de ruta

kerbo quiere hacer lo que hoy hacen por separado Checkstyle, PMD, SpotBugs, ESLint, ruff, mypy,
golangci-lint y los formateadores, con un solo binario, una sola configuración y un solo reporte,
para Java, TypeScript/JavaScript, Python y Go.

## Principios

- **Paridad medida, no aproximada.** Cada regla se compara contra la herramienta que reemplaza
  sobre repos reales y tiene que marcar exactamente lo mismo. Mientras una regla o un lenguaje no
  llegue, se sigue usando la herramienta original: un resultado "parecido" no sirve para un gate.
- **Liviano de verdad.** Un ejecutable nativo, sin runtimes ni plugins, que analiza archivo por
  archivo en paralelo. La meta es menos de 200 MB en el monorepo más grande y segundos en vez de
  minutos.
- **La vara más alta de fábrica.** El preset por defecto es estricto. Se puede aflojar una regla
  por repo, pero siempre con un motivo escrito que aparece en el reporte: nada se afloja en
  silencio.
- **Pensado también para agentes de IA.** Salida compacta y priorizada, identificadores estables
  por problema, arreglos como parche y avisos cuando un cambio esquiva una regla en vez de
  arreglarla.
- **No se reimplementan compiladores ni motores de tests.** Los tipos de TypeScript vienen del
  compilador; cada test lo ejecuta el motor de su lenguaje (JUnit Platform, Node, pytest,
  `go test`), así se comporta igual. Lo que lo rodea (elegir qué correr, compilar, transformar,
  arrancar y reusar procesos) es de kerbo.

## Qué hay hoy (0.1)

Las cinco reglas universales, escritas una vez sobre el árbol de tree-sitter y aplicadas en los
cuatro lenguajes: largo de archivo, largo de función, complejidad ciclomática, complejidad
cognitiva y cantidad de parámetros. Cada lenguaje cuenta como la herramienta que se usa hoy en
él:

| Lenguaje | Imita a |
|---|---|
| TypeScript / JavaScript | ESLint (`complexity`, `max-lines`, `max-lines-per-function`, `max-params`) y sonarjs (`cognitive-complexity`) |
| Python | ruff (`C901`, `PLR0913`) |
| Java | Checkstyle (`CyclomaticComplexity`, `MethodLength`, `ParameterNumber`, `FileLength`) y PMD (`CognitiveComplexity`) |
| Go | golangci-lint (`cyclop`, `gocognit`, `funlen`, `revive`) |

Esas herramientas no cuentan igual, y las diferencias no están documentadas: ESLint suma `??` a la
ciclomática y sonarjs no lo suma a la cognitiva, ruff no cuenta `self` ni `cls`, Checkstyle mide
un método desde la llave y saltea los `@Override`, funlen cuenta las líneas de adentro de la
función con blancos y comentarios. Cada una está fijada en un test.

Velocidad, contra las herramientas corriendo **sólo** estas cinco reglas, con los mismos umbrales
y sobre los mismos archivos (una corrida, 2026-10-19):

| Lenguaje | Herramienta | Tiempo | CPU | Memoria |
|---|---|---|---|---|
| Java, 3.143 archivos | Checkstyle + PMD | 21,2 s | 47,9 s | 740 MB |
| | kerbo | 0,9 s | 2,6 s | 8 MB |
| TypeScript, 2.667 archivos | ESLint + sonarjs | 15,7 s | 27,2 s | 606 MB |
| | kerbo | 1,1 s | 3,0 s | 10 MB |
| Go, 65 archivos | golangci-lint | 0,87 s | 0,90 s | 165 MB |
| | kerbo | 0,04 s | 0,09 s | 5 MB |
| Python, 154 archivos | ruff | 0,06 s | 0,04 s | 18 MB |
| | kerbo | 0,10 s | 0,29 s | 7 MB |

En Python, ruff (también en Rust) sigue ganando en tiempo; kerbo gasta CPU de más arrancando
hilos en repos chicos, que es lo primero a ajustar.

## Qué sigue

En orden. Cada etapa se cierra con la paridad medida y los tests del caso que la motivó.

### 1. Paridad en las dos direcciones y velocidad justa (hecho)

- Arnés que corre kerbo y la herramienta de referencia con los umbrales al mínimo (así cada una
  reporta todas las funciones con su valor) y compara función por función. Prueba que kerbo no
  marca ni de más ni de menos. Cada criterio no documentado que aparece (las IIFE que ESLint
  saltea, el `@overload` de ruff, el anidamiento que sonarjs arrastra después de una flecha en
  un ternario) queda fijado en un test.
- Medir ESLint, ruff, Checkstyle/PMD y golangci-lint corriendo **sólo** estas cinco reglas sobre
  los mismos archivos, y publicar la comparación. Comparar contra el gate entero sería trampa.
- `--max regla=valor` para cambiar un umbral desde la línea de comandos (ya está).

### 2. Lo mínimo para adoptarlo en un repo real (hecho, 0.2)

- **`kerbo.toml`**: presets (`strict`, `recommended`, `minimal`), umbrales por lenguaje y
  excepciones por archivo, siempre con un `reason` obligatorio que aparece en el reporte como
  desvío del preset.
- **Línea de base y modo diff**: `kerbo baseline` guarda lo que ya existe y `kerbo check --diff
  main` marca sólo lo nuevo, así se adopta sin arreglar todo antes.
- **SARIF** y una **GitHub Action** que anota los problemas en el pull request.

### 3. Capa para agentes de IA (hecho: 0.3 y el servidor MCP en 0.4)

- **Antitrampa**: avisar cuando un cambio agrega una supresión (`eslint-disable`, `noqa`,
  `nolint`, `@SuppressWarnings`, `#[allow]`), afloja un umbral en una
  configuración, o borra, saltea o vacía un test.
- `--format agent`: JSON compacto, priorizado y con tope, e identificadores estables por
  problema (regla + símbolo + contexto, no número de línea).
- `kerbo explain <regla>`: por qué existe, ejemplo malo y bueno, cómo se arregla.
- **Modo instantáneo** sobre los archivos recién tocados (meta: menos de 100 ms), un hook listo
  para Claude Code y un **servidor MCP**.
- **Salida para agentes con menos tokens** (pendiente). Hoy cada problema es un objeto JSON
  que repite todas las claves, la ruta del archivo y el texto entero de cómo se arregla. Lo que
  más ahorra no es el formato sino dejar de repetir:
  - **Cómo se arregla, una vez por regla**: el texto de `kerbo explain` va en una sección aparte
    indexada por regla, no en cada problema. Con 30 problemas de la misma regla hoy se manda 30
    veces el mismo párrafo.
  - **Agrupado por archivo**: la ruta aparece una sola vez y debajo van sus problemas.
  - **Identificador corto**: el prefijo más corto del identificador estable que no choca con
    otro dentro del reporte, sin dejar de ser estable entre corridas (un agente lo usa para
    seguir un problema de un cambio al siguiente).
  - **Filas en vez de objetos**: las claves se declaran una vez y cada problema es una fila, al
    estilo de TOON (Token-Oriented Object Notation), que es lo que mejor rinde con listas de
    objetos iguales. YAML no sirve para esto: saca llaves y comillas pero repite las claves igual
    (de 10 a 20 % menos que JSON) y la indentación es frágil.
  - Medir antes de elegir: contar los tokens de JSON, del JSON sin repeticiones y de las filas
    sobre reportes reales (un repo con pocos problemas y uno con cientos), con el tokenizador de
    los modelos que más se usan, y quedarse con lo que menos gaste sin que el agente lo lea peor.
  - El formato ganador pasa a ser `--format agent` y el JSON queda como `--format agent-json`
    para quien lo parsea con código. El mismo formato en el hook y en el servidor MCP.
  - Un test fija la salida y otro el tope de tokens de un reporte de ejemplo, así un cambio que
    la agrande se ve.
- **Cuánto gasta un agente de punta a punta** (pendiente). La velocidad de kerbo ya está medida;
  falta medir lo que paga quien lo usa con un agente: correr el gate, leer la salida, arreglar y
  volver a correr hasta que pase.
  - Arnés: el mismo repo, en el mismo commit y con los mismos problemas, y un agente (Claude Code
    sin interfaz, con el mismo modelo y la misma instrucción) que tiene que dejar el gate en
    verde. Una vez con las herramientas de hoy (Checkstyle, PMD y SpotBugs por Maven o Gradle;
    ESLint; ruff; golangci-lint; clippy) y otra con kerbo (`--format agent`, el modo instantáneo
    y el hook).
  - Medir tokens de entrada y de salida, costo, cantidad de llamadas a herramientas y de corridas
    del gate, tiempo total, si terminó en verde y si hizo trampa (supresiones, umbrales aflojados,
    tests vaciados), revisado con la antitrampa de kerbo en las dos corridas.
  - Varias corridas por caso, porque un agente no responde siempre igual; se publica la mediana y
    la dispersión, con la misma tabla y la fecha que la de velocidad.
  - Casos chicos y grandes por lenguaje: un problema suelto, decenas de la misma regla y un repo
    sin gate que se adopta con línea de base.
  - Lo que se espera ver, y hay que confirmar con números: los logs de Maven y Gradle, el XML de
    SpotBugs y el mismo problema reportado por dos herramientas inflan la entrada; sin modo
    instantáneo el agente vuelve a correr el gate entero después de cada arreglo; sin
    identificadores estables no sabe qué ya arregló.
  - El arnés queda en el repo y corre antes de cada versión, así una regresión en el consumo se
    ve igual que una en la paridad.

### 4. Editor (LSP) e inspecciones de IntelliJ como mapa (el LSP, hecho en 0.5)

- `kerbo lsp`: los problemas de kerbo subrayados mientras se escribe, en IntelliJ (de fábrica en
  Ultimate, con LSP4IJ en Community), VS Code, Neovim y Zed. Los arreglos como acción rápida
  pasan a la etapa 8: ninguna regla de hoy tiene un arreglo mecánico (todas piden un nombre o
  partir una función).
- Correr Qodana Community (las inspecciones de IntelliJ en CI) sobre repos reales y contar qué
  inspecciones aparecen más: esa lista ordena las reglas de las etapas de Java y de flujo de
  datos, y Qodana pasa a ser otra referencia del arnés de paridad. Pendiente: en una máquina de
  8 GB compartida con otros servicios, Qodana sobre un proyecto Maven de 600 archivos no termina
  ni limitado a `src/main` (más de 30 minutos, cuatro núcleos y la máquina en swap). Necesita una
  máquina dedicada con 16 GB.

### 5. Más reglas universales y Rust (hecho: las reglas en 0.6, Rust en main)

- Anidamiento (hecho para TypeScript y Go, con paridad contra `max-depth` de ESLint y
  `max-control-nesting` de revive; Java mide `if`, `for` y `try` por separado y va con las reglas
  de Checkstyle).
- Números mágicos (hecho en los cuatro lenguajes, con paridad contra `no-magic-numbers` con las
  opciones del estándar, `MagicNumber` de Checkstyle, mnd y `PLR2004` de ruff).
- Nombres cortos (hecho en los cuatro lenguajes, con paridad contra `id-length` de ESLint,
  `LocalVariableName` de Checkstyle, varnamelen y los nombres ambiguos de ruff).
- Bloques duplicados (hecho para Java y Python, con paridad contra CPD de PMD 7.28: el mismo
  algoritmo de coincidencias, el orden en que CPD recorre los grupos y los mismos tokens, también
  sobre Guava y la biblioteca estándar de Python; y para Go, con paridad contra `dupl`: el árbol
  de go/ast armado desde tree-sitter y el mismo árbol de sufijos, por paquete; y para TypeScript,
  con paridad contra `no-identical-functions` y `no-duplicated-branches` de sonarjs, también
  sobre Next.js, React DOM, ESLint y zod). El modo instantáneo y el editor sólo comparan los archivos
  que miran; el repo entero, `kerbo check`.
- Código comentado (hecho para Python, con paridad contra ERA001 de ruff, que es la regla del
  estándar, también sobre la biblioteca estándar entera; y para Go, con paridad contra
  `commentedOutCode` de gocritic: los mismos grupos de comentarios que arma go/parser y la misma
  prueba de que compilan como sentencia, también sobre 2.500 archivos de la biblioteca estándar). En TypeScript el estándar no lo
  revisa (`no-commented-code` de sonarjs no está en `recommended`) y en Java ninguna herramienta
  del estándar lo hace: ahí no hay contra qué medir.
- Comentarios que narran lo obvio: ninguna herramienta lo detecta, así que no hay paridad posible.
  Pasa a la capa para agentes, donde el criterio puede ser propio.
- **Rust** como quinto lenguaje (hecho, con paridad contra clippy 0.1.99: `too_many_lines`,
  `too_many_arguments`, `cognitive_complexity` y `excessive_nesting`, que son las reglas del
  estándar; la complejidad se cuenta como clippy, sobre el código ya bajado a HIR, y el largo sobre
  el texto). Cero diferencias en kerbo mismo, que ahora se analiza solo. En ocho crates de
  terceros (aho-corasick, clap_builder, ignore, indexmap, regex-syntax, serde_json, toml_parser y
  winnow, unos 31.000 avisos) lo que difiere es lo que no se ve sin compilar: macros propias o de
  otros crates que esconden un `if`, código escrito adentro de una macro, un `#[cfg]` adentro de
  una función y closures que devuelven `Result` sin decirlo.
- Paridad total en Rust (pendiente). Lo que falta se puede cerrar sin compilar:
  - **Expansor de `macro_rules!`**: leer las definiciones de todo el crate antes de analizar y
    expandir cada invocación como rustc (los fragmentos `expr`, `ident`, `ty`, `tt` y las
    repeticiones), para contar lo que la macro esconde y medir el código que se escribe adentro
    de ella (`quickcheck! { fn .. }`). Es la mayor parte de lo que difiere. Pide que el análisis
    deje de ser sólo archivo por archivo: el modo instantáneo, el hook y el editor tienen que
    conocer las macros del crate, con una caché para no releerlas en cada cambio.
  - **Macros de otros crates**: las `macro_rules!` de las dependencias, leídas de su código en la
    caché de cargo (`~/.cargo/registry/src`) con el mismo expansor. Las macros procedurales
    (`derive`, `#[tokio::main]`, `automod::dir!`) quedan afuera: hay que compilarlas.
  - **`#[cfg]` evaluado**: decidir qué código compila con una plataforma y unas features
    configurables en `kerbo.toml` (por defecto, la máquina donde corre y todas las features), con
    los `cfg` que emite el `build.rs`, para sacar lo que clippy no ve, también adentro de una
    función.
  - Lo que no se puede sin tipos: el `Result` que devuelve un closure sin anotarlo y los módulos
    que declara una macro procedural. Ahí la referencia sigue siendo clippy.

### 6. Distribución (hecho: binarios, crates.io y Homebrew, falta la primera publicación)

- Binarios para macOS (x86 y ARM), Windows y Linux ARM, armados sólo al publicar una versión
  (hecho: el workflow de release corre con un tag `v<versión>` que tiene que coincidir con el
  Cargo.toml, compila cada target en un runner nativo, sin cross, y sube los archivos con su
  sha256 al release; el instalador elige el de la máquina, también bajo Rosetta).
- `cargo install kerbo` desde crates.io, y Homebrew (hecho: el paquete lleva sólo el código, los
  tests, el README y la licencia, y pasa `cargo publish --dry-run`; la fórmula usa los binarios
  del release y el workflow le pone los sha256 de cada versión). Pendiente: crear el tap
  `mati-caliz/homebrew-tap` y cargar los secretos `HOMEBREW_TAP_TOKEN` y `CARGO_REGISTRY_TOKEN`;
  sin ellos el workflow deja la fórmula como artefacto y no publica en crates.io.
- Mensajes, README y sitio en inglés además de español.

### 7. Caché y rendimiento

- Caché por contenido de archivo, comprimida y con tope de tamaño: sólo se analiza lo que
  cambió. Es lo que hace posible el modo instantáneo en repos grandes.
- Una sola pasada por función para todas las métricas, en lugar de una por métrica.
- Un demonio por máquina, con los árboles y la caché en memoria, compartido por el hook, el
  servidor MCP, el LSP y `kerbo check`: varias sesiones de agentes en el mismo repo no repiten el
  trabajo, y el hook responde sin arrancar un proceso por edición.

### 8. Java sintáctico

- Las reglas de forma de Checkstyle y PMD, con paridad medida. Es el hueco más grande: no hay un
  linter rápido de Java.
- Las que tienen un arreglo mecánico (imports sin usar, `final` que falta, modificadores
  redundantes) lo traen como acción rápida del LSP y como parche en `--format agent`.

### 9. Flujo de datos sin compilar

- Lo de SpotBugs sobre el código fuente: nulos, recursos sin cerrar, excepciones tragadas.
- Contaminación de entradas hacia SQL, comandos, rutas y HTML, en todos los lenguajes.

### 10. Go, Python, TypeScript y shell completos

- Cada lenguaje contra su herramienta (golangci-lint, ruff, ESLint), regla por regla.
- Shell contra ShellCheck.
- Código muerto entre archivos: exports que nadie importa y dependencias declaradas que nadie
  usa, con paridad contra knip, vulture y depcheck donde las haya.

### 11. Tests y chequeos inteligentes, dependencias y secretos

- Correr sólo los tests que afecta el cambio y reusar resultados.
- Primero medir, por lenguaje, cuánto de una corrida con un cambio chico es arranque y cuánto son
  los tests.
- Runner propio sobre el motor de cada lenguaje, para bajar ese arranque sin cambiar cómo se
  comporta un test:
  - Java: el launcher de JUnit Platform en una JVM que queda viva, sin pasar por Gradle ni Maven,
    compilando sólo las clases cambiadas; los contextos de Spring quedan en caché entre corridas.
  - JavaScript/TypeScript: transformación propia con caché por contenido y workers de Node
    reusados.
  - Python: un proceso con los imports ya hechos que lanza cada corrida con un fork.
  - Go: `go test` ya compila y guarda resultados en caché; sólo la selección de tests.
- Procesos vivos con tope de memoria y servicios de los tests (bases en contenedores) reusados en
  vez de levantados de cero.
- Los chequeos de tipos en caliente, sin reimplementarlos: `tsc` incremental y sólo en los
  paquetes del monorepo que toca el cambio, y el demonio de mypy (`dmypy`) vivo entre corridas.
- **Mutation testing sólo sobre lo cambiado**: con el mapa de qué test ejecuta qué código, mutar
  las líneas del cambio y correr sólo los tests que las cubren. Marca el test que pasa sin
  verificar nada, que es la trampa típica de un agente.
- `kerbo gate`: el gate entero (análisis, tipos, tests, cobertura) con un planificador propio que
  decide el orden y cuántos pasos en paralelo según la memoria libre.
- Vulnerabilidades de dependencias con OSV y secretos versionados.

### 12. Migraciones y contenedores

- **Migraciones peligrosas** en drizzle, Prisma, Flyway y Alembic: índices sin `CONCURRENTLY`,
  `NOT NULL` sin default, cambios de tipo en tablas grandes, renombres de algo que el código
  desplegado todavía usa. Lo que bloquea una tabla en Postgres se marca antes de llegar a
  producción.
- **Dockerfile** contra hadolint, y reglas para compose: límite de memoria en cada servicio y
  healthchecks con `start_period` e intervalos razonables.

### 13. Formateo propio

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
