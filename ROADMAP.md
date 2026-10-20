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
- `--format agent`: salida compacta, priorizada y con tope, e identificadores estables por
  problema (regla + símbolo + contexto, no número de línea).
- `kerbo explain <regla>`: por qué existe, ejemplo malo y bueno, cómo se arregla.
- **Modo instantáneo** sobre los archivos recién tocados (meta: menos de 100 ms), un hook listo
  para Claude Code y un **servidor MCP**.
- **Salida para agentes con menos tokens** (hecho). Antes cada problema era un objeto JSON que
  repetía todas las claves, la ruta del archivo y el texto entero de cómo se arregla. Ahora
  `--format agent` es texto al estilo de TOON (Token-Oriented Object Notation):
  - **Cómo se arregla, una vez por regla**, en una sección `fix` al final, sólo de las reglas que
    aparecen en lo mostrado.
  - **Agrupado por archivo**: la ruta va una vez y debajo sus filas; los archivos salen en el
    orden de su problema más urgente y las trampas siguen primero.
  - **Filas en vez de objetos**: las columnas (`id,line,end,rule,value,max`) se declaran una vez
    en el encabezado y cada problema es una fila separada por comas.
  - **Identificador corto**: un hash del identificador estable (FNV-1a de 64 bits con la mezcla
    final de MurmurHash3, así identificadores casi iguales no comparten prefijo) y de ese hash el
    prefijo más corto, de 4 caracteres como mínimo, que no choca con otro problema del reporte
    entero (no sólo de lo mostrado). Un mismo problema da siempre el mismo hash y si hace falta un
    carácter más el id viejo sigue siendo su prefijo, así un agente lo sigue de un cambio al otro.
    El JSON de `--format json` sigue trayendo el identificador completo.
  - `--format agent-json` es lo mismo en JSON sin repeticiones (agrupado por archivo y con `fix`
    por regla), para quien lo parsea con código. El hook de Claude Code (después de su
    encabezado) y `kerbo_check` del servidor MCP devuelven `--format agent`.
  - Medido (2026-10-20) con `scripts/count-tokens.py`: o200k_base (GPT-4o y posteriores) y
    cl100k_base (GPT-4) de tiktoken, y el tokenizador público de Claude anterior a Claude 3
    (`Xenova/claude-tokenizer`; el de los Claude actuales no se puede correr local). Reportes del
    preset `strict` sobre pkg/errors (34 problemas), google/uuid (159), express (113), cobra (216)
    y fastapi (421), con el tope de 30 que usa kerbo y sin tope. Tokens de o200k (cl100k da casi
    lo mismo; el de Claude, hasta 17 % más en el JSON y lo mismo en las filas):

    | Repo | Problemas | JSON de antes | JSON sin repeticiones | Filas |
    |---|---|---|---|---|
    | pkg/errors | 34 | 1.631 | 1.029 | 701 |
    | google/uuid | 159 | 1.720 | 1.028 | 710 |
    | express | 113 | 1.719 | 1.179 | 802 |
    | cobra | 216 | 1.785 | 1.076 | 714 |
    | fastapi | 421 | 2.388 | 997 | 678 |
    | fastapi sin tope | 421 | 31.332 | 12.961 | 8.393 |
    | cobra sin tope | 216 | 12.846 | 6.614 | 4.203 |

    Las filas gastan entre 2,1 y 3,7 veces menos que el JSON de antes y un tercio menos que el
    JSON sin repeticiones; cuanto más se repite una regla, más ahorra.
  - Un test fija la salida entera de un reporte chico y otro pone un tope de 650 tokens a uno de
    48 problemas en 8 archivos. Como en los tests no hay tokenizador, se cuentan las piezas que
    separa el pretokenizador de los modelos (palabras, grupos de hasta tres dígitos y cada signo),
    que sobreestima a o200k; hoy da 619.
  - Pendientes:
    - Comprobar que el agente lo lee igual de bien que el JSON (que no confunde columnas ni
      pierde problemas): va con el arnés de punta a punta de abajo.
    - Ninguna fila trae el símbolo (la función o la clase) ni el mensaje: el agente se ubica por
      línea. Ver con el arnés si agregar el símbolo ahorra lecturas de archivo.
    - El hook corta en 30 problemas, como `--format agent` (antes los mostraba todos), y queda
      así: corre después de cada edición, así que el agente ve los siguientes apenas arregla los
      primeros, las trampas van siempre adelante y `more` dice cuántos quedan. Mostrar todos sólo
      engorda el contexto cuando un archivo grande arranca con cientos. El arnés de punta a punta
      lo confirma o lo revierte.
    - Medir con el tokenizador de los Claude actuales (por la API de conteo de tokens) cuando el
      arnés la use de todos modos.
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

### 4. Editor (LSP) e inspecciones de IntelliJ como mapa (hecho: el LSP en 0.5, el mapa en main)

- `kerbo lsp`: los problemas de kerbo subrayados mientras se escribe, en IntelliJ (de fábrica en
  Ultimate, con LSP4IJ en Community), VS Code, Neovim y Zed. Los arreglos como acción rápida
  llegaron con las reglas de forma de Java de la etapa 8, que son las primeras con un arreglo
  mecánico.
- Correr Qodana Community (las inspecciones de IntelliJ en CI) sobre repos reales y contar qué
  inspecciones aparecen más (hecho, 2026-10-20): esa lista ordena las reglas de las etapas 8 y 9.
  Qodana for JVM Community 2026.2.4 con el perfil `Default` del IDE, que es lo que IntelliJ
  subraya al escribir (`qodana.starter` y `qodana.recommended`, los perfiles de fábrica de
  Qodana, dan 11 y 21 problemas en un repo de 640 archivos: muy poco para ordenar nada), sobre
  tres servicios Spring Boot: dos Maven de 640 y 3.060 archivos y uno Gradle de 618. 5.479
  problemas en 63 inspecciones:

  | Inspección | Problemas | Repos | Etapa |
  |---|---|---|---|
  | Declaración sin usar (`unused`) | 3.919 | 3 | 9 |
  | Getters y setters que podría generar Lombok | 384 | 2 | — |
  | `getFirst()` en vez de `get(0)`, `List.of`, `Math.clamp`, bloques de texto | 362 | 3 | 8 |
  | Parámetro que siempre recibe lo mismo, retorno que nadie usa o siempre igual | 259 | 3 | 9 |
  | Valor por defecto repetido en una anotación, `throws` de más, escapes Unicode de más | 134 | 3 | 8 |
  | Anotaciones de nulabilidad que no coinciden con el método que redefinen | 86 | 3 | 9 |
  | Nulos y flujo de datos (`DataFlowIssue`, `Optional.get()` sin chequear, `ConstantValue`) | 59 | 3 | 9 |
  | Tipo privado expuesto en una firma pública | 33 | 3 | 8 |
  | Recursos sin `try` con recursos | 10 | 2 | 9 |

  Lo que la tabla dice para kerbo:
  - `unused` es el 72 % y casi todo es ruido: Community no trae el soporte de Spring ni de JPA
    de Ultimate, así que cada bean sin `new` y cada campo de entidad que llena Hibernate cuenta
    como sin usar. Una regla de código muerto en Java sin entender `@Component`, `@Bean`,
    `@Entity` y los controllers sería igual de inútil.
  - Lo más frecuente que kerbo puede hacer sin tipos es modernizar la API (`getFirst()` y
    `getLast()` solas son 251) y sacar redundancias, y todo tiene arreglo mecánico: va primero
    en la etapa 8.
  - En flujo de datos los nulos le ganan a los recursos 145 a 10, y las excepciones tragadas no
    aparecen (el perfil `Default` no las revisa): la etapa 9 arranca por nulos.

  Cómo correrlo, porque lo de fábrica da un resultado que parece válido y no lo es:
  - Sin `projectJDK` en `qodana.yaml`, la imagen sólo tiene el JDK del IDE, analiza con un nivel
    de lenguaje viejo y reporta miles de errores de sintaxis y de API que no existen.
  - Un proyecto con archivos de Eclipse (`.project`) además del `pom.xml` no abre sin
    `--property=project.open.type=Maven`.
  - No montar el `~/.m2` del usuario: Qodana le escribe un `settings.xml` con su propio
    repositorio local. La caché va en un volumen en `/data/cache`.
  - Con 6 núcleos y 12 GB, abrir el proyecto (importar Maven o Gradle) lleva de 3 a 10 minutos
    en frío y el análisis de 1 a 3,5 minutos; el repo de 3.060 archivos usa 5 GB. En la
    máquina de 8 GB compartida no terminaba.

  Pendiente: sumarlo al arnés de paridad como referencia cuando haya una regla de Java que
  coincida con una inspección.

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
  - **Expansor de `macro_rules!`** (hecho). Antes de medir un archivo se leen las definiciones
    de todo el crate (una vez por corrida; el editor y el hook vuelven a leerlas sólo si cambió
    algún archivo) y cada invocación se expande como rustc: todos los fragmentos, las
    repeticiones con separador y las macros recursivas. Cada token recuerda si lo escribió el
    usuario en la invocación o sale de la definición, porque clippy decide con eso: la
    complejidad de una función cuenta todo lo que deja la expansión; una función que arma la
    macro con su propio texto mide largo y parámetros en la definición (un solo aviso aunque se
    invoque varias veces, y sólo si alguien la invoca) y no mide complejidad; lo escrito entero en
    la invocación se mide como código propio; el anidamiento deja de bajar en el primer bloque
    que arma la macro. Una macro definida adentro de una función o de un módulo de tests no se
    aplica fuera de él, y las de otros archivos llegan si se exportan, si su módulo tiene
    `#[macro_use]` o si están en la raíz del crate. En los ocho crates de terceros (27.300
    avisos de clippy) las diferencias bajan de 1.391 a 1.242; en un crate armado con un caso por
    criterio, de 52 a 0, y kerbo mismo sigue en 0. Lo que queda no es de este expansor: casi todo
    es código que clippy no compila (`#[cfg]` apagados y ejemplos con features), y el resto,
    macros de dependencias (`snapbox`, `proptest` y el `quickcheck!` que invoca indexmap desde
    su propia macro) y closures que devuelven `Result` sin decirlo. Analizar un crate con muchas
    macros tarda hasta el doble que antes. En la caché por contenido, la clave de un archivo de
    Rust suma una huella del texto de todo el crate y de la ruta del archivo en él.
  - Pendientes del expansor:
    - Bajar el costo: un crate con muchas macros tarda hasta el doble (winnow, de unos 110 a
      200 ms; serde_json, de 85 a 160 ms) y un archivo suelto suma unos 8 ms; casi todo es
      expandir.
    - La huella del crate en la clave de la caché es gruesa: cualquier cambio en un archivo del
      crate invalida todos los demás archivos de Rust de ese crate. Afinarla a las macros que el
      archivo usa de verdad (y a las invocaciones que arman funciones) devolvería la caché en
      caliente en crates que se editan seguido.
    - Casos sin cubrir: una función armada por una macro cuyos extremos escribió el usuario pero
      con el `fn` de la definición, y macros no compartidas que invocan funciones-macro de otro
      archivo.
    - El arnés de paridad contra clippy (el crate con un caso por criterio y el script que compara
      los ocho crates de terceros) quedó afuera del repo: sumarlo, como el resto de los arneses.
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
- Pendientes de la distribución:
  - Publicar el primer tag: hasta entonces `cargo install kerbo` y `brew install
    mati-caliz/tap/kerbo`, que ya figuran en el README y en el instalador, no funcionan, y el
    workflow de release no corrió nunca en GitHub Actions.
  - El sitio con el instalador para Linux, macOS y Windows, Homebrew y `cargo install kerbo`
    (hecho).
  - Binario para Windows ARM e instalador en PowerShell para quien no tiene el bash de Git.
- Mensajes, README y sitio en inglés además de español (hecho: `--lang`, `lang` en `kerbo.toml`
  o el locale del sistema, con español por defecto; `README.en.md` y el sitio en `/en/`).
- Pendientes del inglés:
  - El sitio pasó a tener `styles.css`, `copy.js` y la carpeta `en/` además de `index.html`: si
    el deploy copia sólo `index.html`, hay que sumarlos (en el repo no hay configuración de
    deploy para confirmarlo).
  - Plurales en español (hecho: el resumen, la línea de base, el hook y el aviso de tests
    borrados dicen "1 archivo", "1 problema" y "1 desvío").
  - El error "tree-sitter no devolvió un árbol" y el nombre de las funciones sin nombre (hecho:
    los dos pasan por el catálogo; el identificador estable sigue usando "(anónima)", así las
    líneas de base existentes no cambian, y un test lo fija).
  - `README.en.md` y `site/en/` se mantienen a mano: un cambio en la versión en español tiene que
    pasar también a la otra. Un test o un chequeo del gate que compare las secciones de los dos
    README avisaría cuando se desfasan.

### 7. Caché y rendimiento

- Caché por contenido de archivo, comprimida y con tope de tamaño: sólo se analiza lo que
  cambió (hecho). Guarda lo que mide el análisis de cada archivo, también los tokens y nodos que
  usan los duplicados, y en cada corrida recombina lo guardado con lo nuevo, así los duplicados
  entre archivos dan lo mismo. La clave es el contenido, el lenguaje, los umbrales efectivos y la
  identidad del binario; la ruta no, así que se comparte entre clones. 256 archivos con desalojo
  del menos usado hasta 256 MB, LZ4, suma de control por entrada y escritura atómica; si se rompe,
  se ignora. `--no-cache` la apaga. La usan `kerbo check`, `kerbo baseline` y el servidor MCP; el
  hook y el editor miran un solo archivo y no la necesitan todavía (sí cuando el expansor de
  macros tenga que conocer el crate entero: su huella va a la clave y las definiciones de cada
  archivo, como otro tipo de entrada en el mismo almacén).

  Con todas las reglas del preset `strict`, en un contenedor con 4 núcleos (una corrida,
  2026-10-20; la máquina se compartía con otras tareas, así que entre corridas iguales hay hasta
  un 30 % de diferencia):

  | Repo | Corrida | Tiempo | CPU | Memoria |
  |---|---|---|---|---|
  | Guava (Java), 3.277 archivos | sin caché | 9,2 s | 32,1 s | 282 MB |
  | | en frío (llenando la caché) | 7,5 a 9,3 s | 28,3 a 35,6 s | 275 MB |
  | | en caliente | 0,37 s | 0,46 s | 202 MB |
  | | con un archivo editado | 0,44 s | 0,54 s | 204 MB |
  | VS Code (TypeScript), 15.395 archivos | sin caché | 32,2 a 36,5 s | 126 a 145 s | 339 MB |
  | | en frío (llenando la caché) | 34,4 a 45,4 s | 134 a 178 s | 420 MB |
  | | en caliente | 0,45 s | 0,76 s | 111 MB |
  | | con un archivo editado | 0,46 s | 0,75 s | 112 MB |

  Los reportes en caliente son idénticos a los de sin caché (49.492 y 83.598 problemas). La caché
  ocupa 7,2 MB para Guava y 8,1 MB para VS Code. Llenarla no se distingue del ruido. Lo que queda
  en caliente es recorrer el árbol, leer y hashear cada archivo y comparar los duplicados; la
  memoria de Guava es la de los tokens de CPD, que se recombinan enteros en cada corrida.
- Pendientes de la caché:
  - Dos procesos que escriben el mismo archivo de la caché a la vez (hecho: antes de escribir un
    shard se relee el del disco y se combinan las entradas sanas, con la marca de uso más nueva).
    Queda una ventana chica entre releer y renombrar; cerrarla del todo pide un lock o el demonio.
  - El tope configurable (hecho: `max_megabytes` en `[cache]` de `kerbo.toml` y
    `KERBO_CACHE_MAX_MB`, que manda; 256 MB de fábrica). La caché es una por usuario y la comparten
    los proyectos, así que el tope que vale es el del último que escribe.
  - El hook y el editor todavía no la usan; la necesitan cuando llegue el expansor de macros.
  - La memoria en caliente de Guava (202 MB) es casi toda tokens de CPD recombinados enteros:
    guardar la huella de cada archivo y recombinar sólo los grupos que tocan lo que cambió.
- Perfilar la corrida en frío de VS Code (hecho, 2026-10-20, con el VS Code de ese día: 15.407
  archivos y 203 MB de código). Pesaban las funciones repetidas de TypeScript
  (no-identical-functions): cada función se comparaba con todas las anteriores y en cada
  comparación se volvían a sacar los tokens de las dos, cuadrático en funciones por archivo;
  `agentService.ts` (750 KB) tardaba 4,4 s y ahora 0,26 s. Ahora cada cuerpo se tokeniza una vez y
  se busca por tipo y tokens en un mapa. Sin caché, en 4 núcleos y con el mismo reporte (84.292
  problemas): de 36,2 s y 140 s de CPU a 25,6 s y 99 s. Lo que queda es parejo, unos 0,5 s de CPU
  por MB (la mitad que Guava por MB), y el archivo más pesado es un fixture de 8 MB
  (`test-checker.ts`, 2,7 s, casi todo el parseo y las métricas). Los números de la tabla de arriba
  son de antes del arreglo.
- El gate en un checkout de Windows con `core.autocrlf=true` (hecho: el `.gitattributes` deja los
  archivos de texto con LF, y el README dice que desde Git Bash se corre con
  `MSYS_NO_PATHCONV=1`, porque el script se copia de afuera y no se edita). Un checkout hecho
  antes del `.gitattributes` conserva los CRLF hasta `git rm --cached -r . && git reset --hard`.
- Una sola pasada por función para todas las métricas, en lugar de una por métrica.
- Un demonio por máquina, con los árboles y la caché en memoria, compartido por el hook, el
  servidor MCP, el LSP y `kerbo check`: varias sesiones de agentes en el mismo repo no repiten el
  trabajo, y el hook responde sin arrancar un proceso por edición.

### 8. Java sintáctico (empezada)

- Las reglas de forma de Checkstyle y PMD, con paridad medida. Es el hueco más grande: no hay un
  linter rápido de Java.
- Las que tienen un arreglo mecánico (imports sin usar, `final` que falta, modificadores
  redundantes) lo traen como acción rápida del LSP y como parche en `--format agent` (hecho
  para las primeras seis, 2026-10-20):
  - El estándar es `sun_checks.xml` de Checkstyle 14.3, el mismo del que ya salen `MagicNumber`
    y los largos: es la única configuración de referencia que pide todo esto. `google_checks.xml`
    sólo trae `AvoidStarImport`, y el quickstart de PMD 7 tiene `UnnecessaryImport`,
    `UnnecessaryModifier` y `ClassWithOnlyPrivateConstructorsShouldBeFinal` pero no pide `final`
    en los parámetros.
  - Seis reglas: `unused-import` (`UnusedImports` con `processJavadoc` apagado, como en
    `sun_checks`), `redundant-import` (`RedundantImport`), `star-import` (`AvoidStarImport`),
    `redundant-modifier` (`RedundantModifier`), `final-parameter` (`FinalParameters`) y
    `final-class` (`FinalClass`). Todas menos `star-import` traen el arreglo: borrar el import o el
    modificador, o insertar `final`. `recommended` apaga `final-parameter` y `minimal` deja sólo
    los imports sin usar y repetidos.
  - El arreglo es un tipo propio (`src/fix.rs`) que sirve a cualquier regla: ediciones de rango y
    el bloque de diff unificado que resulta, calculado durante el análisis y guardado en la caché
    por contenido junto con lo demás del archivo. El LSP lo ofrece como `quickFix` (y ahora guarda
    el texto de cada documento abierto para responder `textDocument/codeAction`); `--format agent`
    lo manda en el campo `patch`.
  - Paridad: cero diferencias, por archivo, línea, columna y nombre, contra Checkstyle 14.3 con
    sólo esos seis chequeos y las propiedades de `sun_checks`, sobre Guava (3.277 archivos),
    spring-boot (8.708), commons-lang (630) y junit4 (473): 74.892 parámetros sin `final`, 2.184
    imports sin usar, 465 clases que deberían ser finales, 250 modificadores redundantes, 5 imports
    con comodín y 1 import redundante. Correr `sun_checks.xml` entero da las mismas cuentas. Lo que
    no se ve a simple vista quedó fijado en `src/lint/fixtures/`, cada archivo con su salida de
    Checkstyle al lado:
    - Un tipo calificado con argumentos cuenta como uso del último nombre sólo con un nivel antes
      (`Map.Entry<K, V>` usa `Entry`; `java.util.List<String>` no usa `List`).
    - `FinalClass` registra la superclase de una clase anónima con el primer hijo del nodo que
      envuelve al `new`: en un ternario, un cast, paréntesis, una asignación o una cadena de
      llamadas toma otra cosa y la clase base se marca igual; con `extends Red<B>.Orange` el nombre
      se rompe y `Orange` también se marca.
    - tree-sitter-java 0.23 no entiende una anotación de tipo antes de `...`
      (`Object @Nullable ... args`): esa lista de parámetros se vuelve a partir a mano, sin contar
      comentarios.
    - Las columnas cuentan bytes, como en el resto de las reglas; Checkstyle expande los tabs a 8
      (junit4 los usa) y cuenta caracteres. Comparando con esa conversión no queda ninguna
      diferencia.
  - Velocidad: en commons-lang, con y sin las seis reglas, la corrida entera da entre 5,1 y 7,5 s
    en las dos variantes; no se distingue del ruido. Guava sigue en 9 s sin caché.
- Pendientes de las reglas de forma, por frecuencia en `sun_checks.xml` entero sobre los mismos
  cuatro repos (sin contar las que ya hace kerbo):
  - `Indentation` (696.473) y `LineLength` (228.941): son de formato y van con la etapa 13; sin
    medirlas contra el formateador del proyecto marcarían casi todo Guava, que usa
    google-java-format.
  - Javadoc: `JavadocVariable` (29.407), `MissingJavadocMethod` (27.721), `JavadocMethod`
    (19.370), `JavadocType` (1.921), `JavadocPackage` (1.508), `InvalidJavadocPosition` (13).
    Piden leer el Javadoc como árbol; tree-sitter lo deja como un comentario.
  - `DesignForExtension` (26.914), `HiddenField` (12.474), `MethodName` (9.239),
    `FileTabCharacter` (7.717, arreglo mecánico), `WhitespaceAround` (6.533, mecánico),
    `DeclarationOrder` (4.672), `VisibilityModifier` (3.637), `NoWhitespaceAfter` (2.325,
    mecánico), `RightCurly` (2.116, mecánico), `ConstantName` (929), `RegexpSingleline` (518, los
    espacios al final de línea; mecánico), `HideUtilityClassConstructor` (311, mecánico: un
    constructor privado), `TodoComment` (209), `NewlineAtEndOfFile` (179, mecánico),
    `InnerAssignment` (177), `OperatorWrap` (145, mecánico), `AvoidNestedBlocks` (136),
    `MissingSwitchDefault` (104), `NeedBraces` (76, mecánico), `EqualsHashCode` (73) y el resto
    con menos de 70 (`StaticVariableName`, `NoWhitespaceBefore`, `EmptyForIteratorPad`,
    `LeftCurly`, `TypeName`, `EmptyBlock`, `MethodParamPad`, `LocalFinalVariableName`,
    `ModifierOrder`, `IllegalImport`, `MultipleVariableDeclarations`, `MemberName`,
    `GenericWhitespace`, `InterfaceIsType`, `EmptyStatement`, `UpperEll`, `ParameterName`,
    `WhitespaceAfter`, `SimplifyBooleanExpression`, `ArrayTypeStyle`).
  - La misma paridad contra PMD para quien usa el quickstart en vez de Checkstyle:
    `UnnecessaryImport` resuelve tipos con el classpath y no va a coincidir sin las firmas de la
    etapa 9; `UnnecessaryModifier` y `ClassWithOnlyPrivateConstructorsShouldBeFinal`, sí.
  - `kerbo fix` (hecho, 2026-10-20): aplica de una vez todos los arreglos que no se pisan, en el
    CLI, en el servidor MCP (`kerbo_fix`) y como acción `source.fixAll.kerbo` del LSP. Genérico
    sobre el `Fix`: lo que agreguen las reglas de Python, TypeScript o shell con arreglo entra sin
    tocar nada más.
    - Se pisan dos arreglos si alguna de sus ediciones empieza en el mismo punto o se superponen
      sus rangos; uno que termina donde empieza otro no se pisa (`fix::compatible`, en el orden
      del reporte).
    - Por archivo: elegir los compatibles, aplicarlos, volver a analizar el texto en memoria y
      repetir, hasta diez vueltas. Así entra lo que destapa un arreglo (una clase que pasa a
      `final` deja de más el `final` de sus métodos, y se saca en la vuelta siguiente).
    - Antes de escribir se comparan las huellas de lo que queda con las de antes: si aparece un
      problema nuevo, el archivo no se escribe, se listan los problemas y sale con `1`. Se
      escribe a un temporal oculto en la misma carpeta (con los permisos del original) y se
      renombra.
    - `--rule` (repetible) y las rutas filtran; `--dry-run` (alias `--diff`, como ruff) muestra
      el parche sin escribir. El parche es un diff de líneas propio (Myers, sin contexto; si la
      distancia pasa de 1.000 líneas, un solo bloque entre el prefijo y el sufijo comunes), sin
      sumar una dependencia.
  - El LSP respeta `context.diagnostics` (hecho): si el editor manda problemas, sólo se ofrecen
    las acciones rápidas de los de kerbo con el mismo código y la misma línea; si no, las de las
    líneas pedidas. `context.only` filtra por tipo de acción (jerárquico: `source` incluye
    `source.fixAll.kerbo`).
  - Pendientes de `kerbo fix`:
    - El "arreglar todo" del LSP hace una sola vuelta (el protocolo pide ediciones sobre el texto
      original); lo que destapa se ofrece en la siguiente.
    - No detecta un arreglo que rompe la sintaxis: tree-sitter se recupera y el análisis sigue.
      Comparar los nodos `ERROR` de antes y después es barato y cerraría el hueco.
    - El parche no lleva contexto y muestra las líneas sin el `\r` final: en un archivo con CRLF
      `git apply` no lo aplica tal cual.
    - `--diff <referencia>` como en `check` (arreglar sólo lo que toca el cambio) no existe:
      `--diff` es el alias de `--dry-run`.
  - Sumar Qodana al arnés de paridad para las reglas que coinciden con una inspección (imports
    sin usar y modificadores redundantes).
  - El arnés de esta paridad (correr Checkstyle con los seis chequeos, kerbo en JSON y comparar
    con la conversión de tabs) quedó fuera del repo, como los anteriores: falta sumarlo.
- Primero, por lo que más aparece en Qodana: la API nueva que reemplaza a la vieja
  (`getFirst()`, `List.of`, `Math.clamp`, bloques de texto) y las redundancias (valor por defecto
  repetido en una anotación, escapes Unicode de más).

### 9. Inspecciones de IntelliJ propias y flujo de datos sin compilar

Lo que Qodana tarda de 6 a 13 minutos y 5 GB en hacer por repo, en segundos y dentro de la meta
de memoria de siempre, medido sobre los mismos tres servicios y Guava. Casi todo el tiempo de
Qodana se va en abrir el proyecto como un IDE (importar Maven o Gradle, indexar el JDK y cada
jar entero) antes de mirar una línea; kerbo no abre nada, sólo lee lo que cada regla necesita.
Por capas, cada una sirve a las reglas de arriba y se puede publicar sola:

- **Resumen por archivo.** En la misma pasada de tree-sitter que ya existe, cada archivo deja lo
  que declara (clases, métodos con su firma, campos, anotaciones, imports) y lo que referencia
  (llamadas por nombre y cantidad de argumentos, `new`, accesos a campos). Va a la caché por
  contenido, así un archivo editado sólo rehace su resumen. Los árboles no se guardan: el
  resumen son cadenas internadas e índices, unos pocos KB por archivo.
- **Índice del proyecto.** Una fase global que sólo junta resúmenes: resuelve nombres con los
  imports, el paquete y las clases anidadas, y arma quién llama a quién. Donde la resolución es
  ambigua (sobrecargas con el mismo número de argumentos y tipos que no se pueden inferir sin
  compilar), no reporta: un falso positivo rompe un gate, uno que falta no. Habilita parámetro
  que siempre recibe lo mismo, retorno que nadie usa, tipo privado expuesto y código muerto.
  El LSP lo tiene en memoria y lo actualiza por archivo; el modo instantáneo lo lee de la caché.
- **Frameworks como tablas.** Spring, Jakarta/JPA, Jackson, JUnit y Lombok declarados como datos
  (anotación → efecto: la clase se instancia, el campo se escribe, el método se llama, existen
  estos getters), no como código por framework. Es lo que le falta a Qodana Community y lo que
  hace que hoy el 72 % de lo suyo sea ruido.
- **Firmas de las dependencias, sin compilar.** El classpath se le pide una vez a Maven o Gradle y
  se guarda con la huella de los `pom.xml`, `build.gradle` y lockfiles: si no cambian, no se
  vuelve a preguntar. De cada jar se lee el índice del zip y, sólo de las clases que el código
  usa, la firma y las anotaciones del `.class` (sin bytecode). Los jars de `~/.m2` y de Gradle no
  cambian, así que esa caché va por hash del jar y se comparte entre repos. El JDK sale de
  `ct.sym` para el `release` del proyecto, que da qué API existe en cada versión: lo que a Qodana
  le faltaba cuando analizó con el JDK equivocado.
- **Flujo de datos.** Un grafo de control por método armado desde tree-sitter, con la nulabilidad
  de las anotaciones (también `@NonNullApi` del `package-info`) del proyecto y de las firmas de
  arriba. Entre métodos, sólo con resúmenes ("puede devolver null", "cierra lo que recibe")
  guardados en el índice, sin análisis del programa entero. Primero nulos, después recursos sin
  cerrar y excepciones tragadas (en Qodana los nulos son 145 y los recursos 10).
- Contaminación de entradas hacia SQL, comandos, rutas y HTML, en todos los lenguajes, sobre el
  mismo grafo.
- Paridad contra Qodana con el perfil `Default`, inspección por inspección. Donde Qodana se
  equivoca por no conocer el framework, la diferencia queda fijada en un test como mejora, no
  como desvío.

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

### 12. Migraciones y contenedores (hecho en main)

- **Migraciones peligrosas** en drizzle, Prisma, Flyway y Alembic (hecho): las nueve reglas de
  squawk que cubren lo que bloquea o rompe en Postgres, con sus nombres y su criterio
  (`require-concurrent-index-creation`, `require-concurrent-index-deletion`,
  `adding-required-field`, `adding-not-nullable-field`, `changing-column-type`,
  `renaming-column`, `renaming-table`, `ban-drop-column`, `ban-drop-table`). El SQL pasa por un
  lexer propio (strings, `$$`, comentarios anidados, identificadores entre comillas) y un parser
  de sentencias chico: tree-sitter-sequel pide `cc ~1.2`, que choca con el `cc` del resto y con
  `multiple-versions = "deny"`, y su gramática es SQL genérico. Se toma, como squawk con
  `--assume-in-transaction`, que cada archivo corre en una transacción, porque así lo corren las
  cuatro herramientas en Postgres. Alembic se lee sobre el árbol de Python que ya existe: el
  `upgrade()` de los archivos que importan `alembic`, `op.*` y `batch_op.*` traducidos a la regla
  del SQL que generan (incluido el índice de un `sa.Column(index=True)`), el SQL de cualquier
  `.execute(...)` analizado como migración y las ramas de otro dialecto salteadas.

  Paridad contra squawk 2.70.0 real (en Docker, una corrida, 2026-10-20) sobre 1.848 migraciones
  de lobe-chat (drizzle), documenso, trigger.dev y cal.com (Prisma), conductor (Flyway) y
  awesome-compose: **1.598 de 1.598 avisos, sin sobrantes**, con las nueve reglas presentes.
  Quedan afuera 15 archivos donde squawk da error de sintaxis (MySQL o SQLite). Alembic contra
  squawk sobre el SQL que genera el propio Alembic en modo offline (`upgrade --sql` por revisión,
  aislada) de airflow, superset, mlflow y redash: de 662 revisiones convierten 261 (el resto
  importa la aplicación o consulta la base); por archivo y regla coinciden **117 de 118**, con 4
  de más; contando cada aviso, 182 de 195 y 11 de más. Lo que falta son los `for` que repiten una
  operación por tabla (kerbo marca la llamada una vez); lo que sobra, caminos que el modo
  offline no recorre (`is_offline_mode()`, `try`/`except`, una operación que en offline levanta
  una excepción y corta el resto), donde kerbo marca lo que corre en una migración real.
  1.848 migraciones y 178 Dockerfiles se analizan en 0,08 s; squawk tarda 0,40 s con las
  migraciones solas.
- **Dockerfile** contra hadolint (hecho): 38 reglas DL, las más comunes primero por frecuencia
  en los repos medidos, con el código como nombre (`dl3008`; `kerbo explain DL3008` también vale).
  Parser propio de instrucciones (continuaciones, `# escape=`, comentarios dentro de un `RUN`,
  heredocs, `ONBUILD`, forma JSON) y del shell de cada `RUN`, que separa comandos y banderas como
  ShellCheck: cada comando cuenta dos veces para DL3059 porque ShellCheck lo ve como
  `T_Redirecting` y como `T_SimpleCommand`, `[ ... ]` no es un comando y los pipes adentro de
  `$( )` cuentan para DL4006. `recommended` apaga las de nivel `info` y `minimal` deja sólo las de
  nivel `error`, como hadolint las clasifica.

  Paridad contra hadolint 2.15.1 real (en Docker, con `--disable-ignore-pragma`, una corrida,
  2026-10-20) sobre 178 Dockerfiles de awesome-compose, docker-stacks, airflow, superset, mlflow,
  nextcloud/docker, mastodon, sentry self-hosted, grafana y redash: **439 de 439 avisos de las
  reglas cubiertas, sin sobrantes**. Quedan afuera 23 archivos donde hadolint no parsea (DL1000:
  casi todos un heredoc seguido de otra instrucción). hadolint tarda 2,1 s (7,4 s de CPU, con
  ShellCheck) en esos 178 archivos y kerbo 0,025 s.

  Cubiertas: DL3000, DL3002 a DL3004, DL3006 a DL3009, DL3012 a DL3016, DL3018 a DL3025, DL3027,
  DL3029, DL3032, DL3042, DL3045, DL3047, DL3059, DL3061, DL3062, DL3064, DL3066, DL3067, DL4000,
  DL4001, DL4003, DL4004 y DL4006.
- **docker compose** (hecho, criterio propio porque no hay herramienta de referencia), sobre la
  gramática tree-sitter-yaml 0.7.2 (MIT, pasa cargo-deny) con anclas y `<<:` resueltos:
  `compose-memory-limit` (cada servicio con `image` o `build` necesita
  `deploy.resources.limits.memory` o `mem_limit`; los que usan `extends` y los fragmentos de un
  override, sin `image` ni `build`, se saltean), `compose-healthcheck-start-period` (un
  healthcheck activo sin `start_period`) y `compose-healthcheck-interval` (`interval` entre 5 s y
  5 min, `timeout` menor que el intervalo; sin `interval` vale el de Docker, 30 s). Sobre los 132
  archivos compose de los mismos repos marca 333 avisos en 73 archivos.
- Integración: tres lenguajes nuevos al final de `Language` (`Sql`, `Dockerfile`, `Compose`) y
  dos pilas para `--language` y `kerbo.toml` (`sql`, `docker`); `Language::grammar()` y
  `Language::profile()` pasan a devolver `Option` porque SQL y Dockerfile no tienen gramática de
  tree-sitter ni métricas de código. Las reglas nuevas son tres familias de `RuleId`
  (`Migration`, `Dockerfile`, `Compose`) que se prenden o se apagan; en `Limits` van como bits,
  así el arreglo de umbrales no crece con cada regla. Lo encontrado entra en la caché por
  contenido como una lista más del análisis de cada archivo (versión de formato 3, la siguiente
  a la de las reglas de Java). Textos en español e inglés en `src/i18n/*/deploy.rs` y
  `src/i18n/*/dockerfile.rs`, con `kerbo explain` para cada regla.
- Pendientes:
  - **Renombres que el código desplegado todavía usa**: no hay forma confiable sin compilar. Los
    nombres de columnas pasan por los mapeos del ORM (`@map` de Prisma, `name=` de SQLAlchemy,
    los nombres de drizzle), cambian de caso y coinciden con palabras comunes (`name`, `status`),
    así que buscar el nombre viejo en el código da falsos positivos y negativos. Mientras tanto,
    como squawk, todo renombre se marca. Lo confiable sería leer el esquema del ORM (Prisma,
    drizzle, modelos de SQLAlchemy) y cruzarlo con la migración.
  - El resto de squawk: `adding-foreign-key-constraint`, `constraint-missing-not-valid`,
    `ban-drop-not-null`, `ban-drop-constraint`, `adding-field-with-default` con default volátil,
    `adding-serial-primary-key-field`, `disallowed-unique-constraint`,
    `ban-concurrent-index-creation-in-transaction`, `require-timeout-settings`,
    `transaction-nesting`, `renaming-object` y las de preferencia (`prefer-bigint-over-int`,
    `prefer-text-field`, `prefer-timestamptz`, `prefer-identity`, `prefer-robust-stmts`).
  - Alembic: expandir los `for` sobre listas literales para contar una vez por tabla, seguir las
    funciones auxiliares que llaman a `op` desde fuera de `upgrade()`, y medir paridad sobre las
    401 revisiones que el modo offline no convierte (habría que levantar cada aplicación).
  - Migraciones de Django y Rails (`AddIndex` sin `AddIndexConcurrently`, `add_index` sin
    `algorithm: :concurrently`), con strong_migrations como referencia de Rails.
  - Las reglas de hadolint que faltan (sobre todo DL3046, DL3033 y DL4005) y las de ShellCheck
    sobre el shell de cada `RUN`, que es la etapa 10.
  - Los Dockerfile y compose en carpetas ocultas (hecho: el recorrido entra a `.github/` y
    `.devcontainer/`, para todos los lenguajes, y sigue salteando el resto de lo oculto, como
    `.git/`, `.venv/` o `.next/`, y los archivos ocultos). La lista es corta a propósito: sumar
    una carpeta es sumarla en `src/scan/walk.rs`. Los workflows de GitHub Actions no tienen
    reglas todavía, así que en `.github/workflows/` no hay nada que leer.
  - `squawk-ignore` y `# hadolint ignore=` (hecho, 2026-10-20): se respetan como en la
    herramienta original y la antitrampa los marca como supresión nueva, igual que
    `# shellcheck disable=`. Un solo registro, `SYNTAXES` en `src/suppressions.rs` (comentario,
    marcador y alcance), alimenta las dos cosas; los códigos se comparan con el nombre de la
    regla sin distinguir mayúsculas (`DL3008` con `dl3008`, `SC2086` con la regla de shell que
    se llame `sc2086`), así que sumar una herramienta es sumar una fila.
    - Alcance: `squawk-ignore` y `hadolint ignore=` valen para la línea siguiente que no sea
      blanca ni comentario (se pueden apilar); `squawk-ignore-file` y `hadolint global ignore=`,
      para el archivo entero. Sólo cuentan como comentario al principio de la línea.
    - Lo que sigue a la lista de reglas es el motivo. Cada problema ignorado sale en el log como
      `Dockerfile:3: dl3008 ignorada en el código: <motivo>` (o `sin motivo`) y el resumen suma
      `· N ignorados en el código`; con `--diff`, sólo los de las líneas cambiadas.
    - Para que la antitrampa corra en SQL y Dockerfile, `suppression-added` queda prendida en
      esos lenguajes (el resto de las reglas de código siguen sin medirse ahí).
  - Pendientes de las supresiones en el código:
    - `squawk-ignore-file` sin reglas (todas) y `# shellcheck disable=all` no se reconocen: haría
      falta saber qué familia de reglas es de cada herramienta.
    - ShellCheck: una directiva antes de un comando compuesto vale para todo el bloque y una antes
      del primer comando, para el archivo entero; hoy vale para la línea siguiente. Lo cierra quien
      sume las reglas `SC*`.
    - squawk asocia el comentario a la sentencia siguiente; kerbo, a la línea siguiente: un
      `ALTER TABLE` partido en varias líneas con la violación más abajo no queda cubierto.
    - SARIF no los manda como `suppressions` (`kind: inSource`, con `justification`); GitHub los
      mostraría como resueltos.
  - compose: el límite de CPU, `restart`, imágenes sin versión fija (como DL3006/DL3007) y los
    servicios que dependen de otro sin `condition: service_healthy`.

### 13. Formateo propio

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
