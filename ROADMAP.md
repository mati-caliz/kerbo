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

### 4. Editor (LSP) e inspecciones de IntelliJ como mapa (cerrada: el LSP en 0.5, el mapa en 0.7)

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

  Sumar Qodana al arnés de paridad cuando una regla de Java coincida con una inspección quedó en
  la etapa 8, con esas reglas.

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

### 6. Distribución (cerrada: publicada desde 0.7.0)

- Binarios para Linux, macOS y Windows, en x86 y ARM, armados sólo al publicar una versión
  (hecho: el workflow de release corre con un tag `v<versión>` que tiene que coincidir con el
  Cargo.toml, compila cada target en un runner nativo, sin cross, y sube los archivos con su
  sha256 al release; el instalador elige el de la máquina, también bajo Rosetta).
- `cargo install kerbo` desde crates.io, y Homebrew (hecho: el paquete lleva sólo el código, los
  tests, el README y la licencia, y pasa `cargo publish --dry-run`; la fórmula usa los binarios
  del release y el workflow le pone los sha256 de cada versión).
- Primera publicación (hecho, 0.7.0, 2026-10-21): el workflow corrió entero por primera vez y
  dejó los cinco binarios en el release, la fórmula en `mati-caliz/homebrew-tap` y el crate en
  crates.io. Los secretos son `HOMEBREW_TAP_TOKEN` (fine-grained, sólo escritura de contenido
  sobre el tap) y `CARGO_REGISTRY_TOKEN` (`publish-new` y `publish-update`); crates.io rechaza
  publicar si la cuenta no tiene el mail verificado. 0.4 a 0.6 son releases a mano con sólo el
  binario de Linux.
- El sitio con el instalador para Linux, macOS y Windows, Homebrew y `cargo install kerbo`
  (hecho).
- Windows ARM e instalador en PowerShell (hecho, 2026-10-21): el release suma
  `aarch64-pc-windows-msvc` en el runner `windows-11-arm` (instala rustup si la imagen no lo trae,
  y si falta 7-Zip el zip lo arma PowerShell). `install.sh` lo elige desde el bash de Git en ARM y
  `install.ps1` (`irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex`) instala en Windows x86_64
  y ARM sin bash: verifica el sha256, deja `kerbo.exe` en `%LOCALAPPDATA%\Programs\kerbo` y la
  suma al PATH del usuario. Corre en Windows PowerShell 5.1 y en PowerShell 7. El binario ARM sale
  recién con la próxima versión.
- Mensajes, README y sitio en inglés además de español (hecho: `--lang`, `lang` en `kerbo.toml`
  o el locale del sistema, con español por defecto; `README.en.md` y el sitio en `/en/`).
- Lo que faltaba del inglés:
  - El deploy del sitio copiaba sólo `index.html`, `install.sh` y `llms.txt`: `styles.css`,
    `copy.js`, `install.ps1` y `en/` daban 404 (comprobado el 2026-10-21). El deploy vive en el
    servidor, fuera del repo, y tiene que publicar la carpeta `site/` entera.
  - Plurales en español (hecho: el resumen, la línea de base, el hook y el aviso de tests
    borrados dicen "1 archivo", "1 problema" y "1 desvío").
  - El error "tree-sitter no devolvió un árbol" y el nombre de las funciones sin nombre (hecho:
    los dos pasan por el catálogo; el identificador estable sigue usando "(anónima)", así las
    líneas de base existentes no cambian, y un test lo fija).
  - `README.en.md` y `site/en/` se mantienen a mano (hecho el chequeo: `tests/translations.rs`
    compara los títulos, las tablas y los bloques de código de los dos README, y las secciones,
    filas, bloques de instalación y comandos copiables de los dos sitios; si algo está en un
    idioma y no en el otro, el gate falla).

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
  Pendiente: con `MSYS_NO_PATHCONV=1`, `pwd` devuelve `/c/Users/...` y Docker Desktop a veces
  monta `/src` vacío según la carpeta; con `pwd -W` anda siempre. Como `verify-rust.sh` viene del
  estándar (`quality/sync.sh`), el arreglo va allá y no en este repo.
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
- Python contra ruff (empezada, 2026-10-21): las 55 reglas del conjunto clásico de ruff (`E4`,
  `E7`, `E9` y `F`, el que pide la tarea; ruff 0.17 cambió su conjunto por defecto por uno mucho más
  grande, con `B`, `PL`, `UP`, `RUF` y otros) salvo E902, F407, F621 y F622, con el mismo código en
  minúscula como nombre de regla (`f401`, `e711`), en una familia propia (`PythonRule`).
  - Frecuencia medida sobre los diez repos de abajo, de mayor a menor: F405 4.167, F821 2.194,
    E402 1.799, E741 1.451, F841 1.361, E701 1.283, E712 1.031, E731 684, F401 627, E722 255,
    E721 220, E702 208, F541 176, E711 99, E401 98, F403 97, E713 86, F811 79, F601 53, E703 47,
    E742 35, F402 30, F822 22, F507 22 y el resto con menos de 20.
  - Motor de alcances propio sobre tree-sitter, calcado del modelo semántico de ruff: alcances de
    módulo, clase, función, lambda, comprensión y tipo (PEP 695); cuerpos de funciones y lambdas
    diferidos al final del módulo; anotaciones entre comillas parseadas aparte con la posición
    exacta adentro del string (o la del string si tiene escapes o está concatenado), y anotaciones
    diferidas con `from __future__ import annotations` y en los `.pyi`; `typing.cast`, `TypeVar`,
    `NewType`, `NamedTuple`, `TypedDict`, `TypeForm`, `Literal`, `Annotated` y `TypeAlias`
    resueltos por nombre calificado; ramas de `if`, `try`, `match` y `with` para no marcar
    redefiniciones en caminos excluyentes.
  - Arreglos mecánicos con el `Fix` de siempre (salen como parche en `--format agent`, acción
    rápida del LSP y en `kerbo fix`): F401 (el import entero, el nombre de una lista o `pass` si el
    bloque queda vacío; no en `__init__.py` ni con otra sentencia en la línea), F541, E703, E711,
    E712, E713, E714, F632, F901 y el `as` sin usar de un `except` (F841).
  - `# noqa`, `# noqa: CÓDIGOS` y `# ruff: noqa`/`# flake8: noqa` como ruff: en un string de varias
    líneas o una línea continuada con `\` el `noqa` va en la última; F401 también mira la primera
    línea del import; `# noqa:` sin códigos válidos no silencia nada.
  - Paridad: cero diferencias por archivo, línea, columna, regla y cantidad contra ruff 0.17.1
    (`--isolated --select E4,E7,E9,F`) en requests, fastapi, flask, django, pydantic, sqlalchemy,
    ipython, ansible, sympy y `Lib/` de CPython: 15.916 avisos en unos 11.200 archivos. Lo que queda,
    explicado:
    - 82 archivos de CPython usan `lazy import` (PEP 810, de 3.15) y quedan fuera de la medición:
      tree-sitter-python 0.25 no lo conoce.
    - En los archivos donde tree-sitter devuelve errores no corren las reglas que dependen de los
      alcances (un árbol roto da falsos positivos, que rompen un gate): en CPython son
      `test_compile.py` (un `dedent` adentro de paréntesis) y `test_patma.py`, 103 avisos que ruff
      sí da. Las reglas locales (E70x, comparaciones, f-strings) corren igual.
    - E902 (5 avisos) es un error de lectura de ruff con archivos que no están en UTF-8, no una
      regla.
  - Lo que no se ve a simple vista quedó en `src/lint/python/fixtures/`, cada archivo con la salida
    de ruff al lado: los builtins de 3.11 (sin `PythonFinalizationError`), `__class__` sólo en
    métodos y `__module__`/`__qualname__` sólo en el cuerpo de la clase; la anotación sin valor que
    deja el nombre sin definir en una función pero no en una clase; el `del` condicional (adentro de
    un `if`, `while`, `match` o un `except`); el `global` de una función que define el nombre en el
    módulo cuando se lee la `def`, no cuando corre; los parámetros que se ligan recién al analizar
    el cuerpo (y por eso redefinen un import posterior); las variables de una comprensión dentro de
    un `for` que cuentan como variables del loop; las redefiniciones entre alcances que no miran las
    clases; la resolución de las anotaciones entre comillas que prueba primero el módulo; el
    `@overload` resuelto por nombre calificado (un `typing` reexportado no vale), y F507, que cuenta
    como un solo valor cualquier literal que no sea una tupla.
  - Las columnas cuentan bytes, como el resto de las reglas; ruff cuenta caracteres (la medición
    convierte).
- Pendientes de Python contra ruff:
  - Del conjunto clásico: E902 (no es una regla), F407 (`from __future__` de una característica que
    no existe), F621 y F622 (expresiones con `*` de más): ninguna apareció en los diez repos.
  - El resto del conjunto por defecto de ruff 0.17 (`B`, `PL`, `UP`, `RUF`, `SIM`, `C4`, `PYI`,
    `FURB`, `PERF`, `ASYNC`, `S`, `TRY`, `PT`, `LOG`, `YTT`…), sin medir todavía la frecuencia.
  - Las reglas de alcance en archivos que tree-sitter no parsea, y `lazy import` cuando la
    gramática lo sume.
  - `noqa` no pasa por el registro de supresiones de `src/suppressions.rs`: lo silenciado no aparece
    como suprimido con su motivo.
  - `short-name` de Python marca los mismos `l`, `O` e `I` que `e741`: hoy salen los dos.
  - Arreglos que ruff tiene y kerbo todavía no: E401 (partir el import), F811 (borrar la definición
    pisada), F504/F522/F523 (sacar los argumentos de más) y E731 (pasar la lambda a `def`).
  - El arnés de esta paridad (ruff en JSON contra kerbo en JSON con la conversión de columnas)
    quedó fuera del repo, como los anteriores.
- TypeScript y JavaScript contra ESLint (empezada, 2026-10-21): 20 reglas de `@eslint/js`
  recommended y de `typescript-eslint` recommended sin tipos, con el nombre de ESLint sin el prefijo
  del plugin, en una familia propia (`EslintRule`) y un submódulo propio (`src/lint/ecmascript/`).
  - Frecuencia medida con todo `recommended` de los dos (más `no-var`, `prefer-const`,
    `prefer-rest-params`, `prefer-spread` y `no-non-null-assertion`) sobre next.js, react, VS Code,
    zod y express, de mayor a menor: no-var 79.279, no-unused-expressions 34.661, no-unused-vars
    9.431, prefer-const 8.552, no-require-imports 8.221, no-explicit-any 6.688,
    no-non-null-assertion 2.720, prefer-rest-params 1.780, no-useless-assignment 1.778,
    no-prototype-builtins 1.480, no-redeclare 1.449, no-empty 953, no-useless-escape 711,
    no-fallthrough 700, no-cond-assign 655, no-this-alias 612, ban-ts-comment 552, no-func-assign
    508, prefer-spread 487, no-empty-object-type 293, no-misleading-character-class 290,
    no-constant-condition 248 y el resto con menos de 160. Casi todo `no-var` y
    `no-unused-expressions` sale del código compilado de next.js, que su `.ignore` deja afuera.
  - Hechas: `no-unused-vars` (la de typescript-eslint, también en JavaScript), `prefer-const`,
    `no-var`, `no-unused-expressions`, `no-require-imports`, `no-explicit-any`,
    `no-non-null-assertion`, `prefer-rest-params`, `no-prototype-builtins`, `no-empty`,
    `no-useless-escape`, `no-cond-assign`, `no-this-alias`, `ban-ts-comment`,
    `no-empty-object-type`, `no-case-declarations`, `no-debugger`, `no-duplicate-case`,
    `no-dupe-else-if` y `no-dupe-keys`.
  - Motor de alcances calcado de `@typescript-eslint/scope-manager` sobre tree-sitter
    (`src/lint/ecmascript/scope/`): los mismos alcances (también los de tipos, firmas, tipos
    mapeados y condicionales, enums y namespaces), definiciones de tipo y de valor por separado,
    referencias de tipo, de valor y dobles (`export { x }`), la resolución al cerrar cada alcance en
    el mismo orden que ESLint (el orden de las referencias decide `prefer-const` y la posición de
    `no-unused-vars`), el `arguments` implícito, la referencia a `React` del primer JSX y las marcas
    de `typescript-eslint` (parámetros de firmas y setters, propiedades de parámetro, miembros de
    enum, la clase dentro de su propio alcance, `for (k in o) return`, declaraciones ambientales y
    `.d.ts`).
  - Arreglos con el `Fix` de siempre (parche en `--format agent`, acción rápida del LSP y
    `kerbo fix`): `prefer-const` y `no-var` con las mismas condiciones que ESLint (cada uno contra
    `eslint --fix` en `src/lint/ecmascript/fixtures/*.fixed`), `no-useless-escape` (ESLint sólo lo
    sugiere: kerbo borra la barra) y `no-debugger` cuando la sentencia está sola en una lista.
  - Con la antitrampa: en `--diff` y el hook, un `any` o un `@ts-ignore` agregados salen sólo como
    `type-escape` o `suppression-added`, no repetidos como `no-explicit-any` o `ban-ts-comment` en la
    misma línea (`src/guard/overlap.rs`).
  - Los avisos van a la caché por contenido con el resto del análisis (formato 6).
  - Paridad contra ESLint 10.13 y typescript-eslint 8.72 (parser de typescript-eslint en todos los
    archivos, JSX en los `.js`, `noInlineConfig` y sólo estas reglas; en los `.ts` apagado lo que
    apaga `eslint-recommended`), por archivo, línea, columna, regla y cantidad, en next.js (24.682
    archivos fuera de su `.ignore`), react (4.462), VS Code (722), zod (521) y express (141): 42.488
    avisos iguales, ninguno de más y 587 de menos. Lo que queda, explicado:
    - Archivos que tree-sitter-typescript 0.23 no parsea y kerbo no analiza (un árbol roto da falsos
      positivos): `typeof import('x')` y `import('x').A['b']` en tipos, `let as`, `export type *`,
      `abstract?:` en una interfaz, un template con argumentos de tipo y escapes dentro de un tipo
      de template. Son 521 avisos en unas cuatro decenas de archivos (la mayoría de los tests de
      Copilot en VS Code y algunos de next.js).
    - 15 archivos `.js` de react-devtools con anotaciones de tipos y argumentos de tipo en llamadas
      (`useState<boolean>(false)`): TypeScript los lee en modo JavaScript como comparaciones
      (`useState < boolean > (false)`) y kerbo, que los parsea con la gramática de TSX, los deja
      afuera; son 66 avisos.
    - Para que no sobre nada, el árbol se repara antes de rendirse: un `.js` que la gramática de
      JavaScript no parsea se vuelve a parsear con la de TSX (TypeScript acepta anotaciones en
      JavaScript); los `&` sueltos en JSX (`Vercel & Next.js`, `?a=1&b=2`), que tree-sitter no
      entiende, se blanquean, igual que la varianza `in`/`out` de los parámetros de tipo; y se
      descartan los archivos con sintaxis de Flow que tree-sitter acepta y TypeScript no
      (`(x: T)`, `import typeof`, `<T: U>`, `{| |}`, `declare module.exports`). Un archivo que ESLint
      no parsea tampoco lo analiza kerbo; queda uno en next.js que kerbo sí lee.
    - tree-sitter-typescript aplica el `!` de no nulo a toda la expresión de la izquierda
      (`a || b < c!`) y parte `x satisfies a.b.C` en un acceso a miembro: las dos se corrigen al
      ubicar el aviso.
    - Las columnas cuentan bytes, como el resto de las reglas; ESLint cuenta unidades de UTF-16 (la
      medición convierte).
  - Lo que no se ve a simple vista quedó en `src/lint/ecmascript/fixtures/`, cada archivo con la
    salida de ESLint al lado (`.eslint`) y, para los arreglos, el resultado de `eslint --fix`.
- Pendientes de TypeScript y JavaScript contra ESLint, por frecuencia:
  - `no-useless-assignment` (1.778), `no-fallthrough` (700), `no-unreachable` (104) y
    `no-unassigned-vars` (59): piden el grafo de control de ESLint (`code-path-analysis`); armarlo
    sobre tree-sitter es el paso siguiente.
  - `no-redeclare` (1.449) y `no-func-assign` (508), sólo en JavaScript, sobre el motor de
    alcances que ya existe; `prefer-spread` (487), `no-misleading-character-class` (290),
    `no-constant-condition` (248), `no-constant-binary-expression` (154),
    `no-wrapper-object-types` (152), `no-control-regex` (138), `no-unsafe-function-type` (113),
    `preserve-caught-error` (92), `triple-slash-reference` (70), `no-unsafe-finally` (67),
    `getter-return` (43), `no-extra-boolean-cast` (38) y el resto con menos de 30.
  - `no-undef`: pide las variables globales de cada entorno (navegador, Node, Jest).
  - `no-useless-escape` no mira las expresiones regulares con el flag `v`.
  - Los `.js` con argumentos de tipo en llamadas y los huecos de la gramática de arriba.
  - `eslint-disable` no pasa por el registro de supresiones de `src/suppressions.rs`: hoy no
    silencia nada (la paridad se midió con `noInlineConfig`).
  - Las reglas con tipos de `typescript-eslint` (`recommended-type-checked`): esperan a los tipos
    del compilador.
  - El arnés de esta paridad (ESLint con un `eslint.config.mjs` de sólo estas reglas, kerbo en JSON
    y la conversión de columnas) quedó fuera del repo, como los anteriores.
- Shell contra ShellCheck (empezada, 2026-10-21): `.sh`, `.bash` y los archivos sin extensión con
  shebang de `sh`, `bash`, `dash` o busybox (también por `env`), con tree-sitter-bash 0.25.1
  (MIT). Las 22 reglas más frecuentes de ShellCheck, con el código en minúscula como nombre
  (`sc2086`) en una familia propia (`ShellRule`, en `src/rules/families/shell.rs`): SC2004, SC2005, SC2006, SC2015, SC2016, SC2034, SC2046, SC2059,
  SC2064, SC2068, SC2069, SC2086, SC2116, SC2148, SC2155, SC2162, SC2164, SC2166, SC2181, SC2206,
  SC2207 y SC2223. `kerbo explain SC2086` acepta el código en mayúsculas.
  - La severidad es la de ShellCheck: el preset recomendado deja afuera las de estilo y el mínimo,
    también las informativas.
  - Las partes de ShellCheck que deciden si avisa, portadas de su código: si una expansión está
    libre de comillas, si una palabra es nombre de comando o condición, las opciones de cada
    comando al estilo GNU, BSD o genérico, el dialecto (shebang, `# shellcheck shell=`, extensión) y
    el flujo de datos: un grafo de control con estados que se juntan en ramas, loops, `case` con
    `;&` y `;;&`, subshells y pipelines (con la rareza de que cualquier `set -o` prende `lastpipe`),
    funciones llamadas con su marco de variables locales, la caché de llamadas por dependencias
    como ShellCheck, y el valor de cada variable como vacío, limpio o con espacios para SC2086
    (también los enteros de `declare -i`, `let`, `(( ))`, `read`, `mapfile`, `printf -v` y
    `wait -p`). SC2034 sale sin flujo, como en ShellCheck, y los arreglos asociativos no cuentan
    sus índices como aritmética.
  - Arreglos mecánicos con el `Fix` de siempre (`--format agent`, LSP y `kerbo fix`): comillas en
    SC2086, `$(…)` en vez de backticks en SC2006, `-r` en SC2162 y `|| exit` en SC2164. Las
    violaciones de deploy y de shell guardan su arreglo en la caché (formato 7).
  - `# shellcheck disable=` con el alcance real de ShellCheck, en el registro de
    `src/suppressions.rs` (así lo silenciado entra en la antitrampa con su motivo): antes del primer
    comando vale para el archivo entero, antes de un comando compuesto para todo el bloque, al
    final de una línea no vale, y antes de `elif`, `else` o un caso de `case` tampoco. Acepta
    `SC2086`, `2086`, rangos (`SC2000-SC2099`), `all` y varias claves `disable=` en el mismo
    comentario.
  - Paridad contra ShellCheck 0.11.0 (`koalaman/shellcheck-alpine`) en 1.752 scripts de
    kubernetes (`hack/`, `cluster/`, `build/`, `test/`), moby (`hack/`, `contrib/`), nvm, rustup,
    ohmyzsh, docker-install, acme.sh, bash-it, dokku, neofetch y los scripts de kerbo: 1.680
    avisos, cero diferencias por archivo, línea, columna y regla. Sacando todas las directivas
    `# shellcheck` de esos mismos archivos (para medir las reglas sin que nadie las haya
    silenciado): 3.215 avisos de ShellCheck, 3.210 de kerbo, ninguno de más. kerbo tarda 1,5 s en
    todo el corpus; ShellCheck tarda 18 s sólo en `nvm.sh`.
  - Lo que no se ve a simple vista quedó en `src/shell/fixtures/flow.sh`, con la salida de
    ShellCheck al lado, y en los tests de `src/shell/tests.rs`.
  - tree-sitter-bash tiene dos errores que cambian lo que se lee:
    - En `$A/$B/V` deja un `$` suelto. Antes de analizar, las expansiones pegadas a un separador
      se reescriben como `${NOMBRE}`, se vuelve a parsear y las posiciones, los arreglos y los
      textos vuelven al original.
    - Un comentario entre líneas continuadas con `\` se come el resto del comando ("líneas
      tragadas"). Esos archivos se saltean enteros, como los que tienen nodos de error.
- Pendientes de Shell contra ShellCheck:
  - 55 archivos del corpus (260 avisos de ShellCheck, 375 sin directivas) quedan afuera porque
    tree-sitter-bash no los parsea o se traga líneas; entre ellos el `acme.sh` principal.
  - Sin directivas faltan 5 avisos: 3 SC2034 en `dnsapi/dns_ali.sh`, `deploy/ali_cdn.sh` y
    `deploy/ali_dcdn.sh` de acme.sh (el `$` suelto que la reparación no alcanza, y el archivo se
    saltea), y 2 SC2086 en `nvm.sh` (líneas 5037 y 5202, `EXIT_CODE=$?; return $EXIT_CODE`) sin
    explicar todavía.
  - El resto de las reglas de ShellCheck: SC1090/SC1091 (`source`, que tampoco se sigue), SC2154,
    SC2329, SC2031, los SC3xxx de `sh` y las demás, sin medir todavía la frecuencia.
  - ShellCheck sobre el shell de cada `RUN` de los Dockerfile.
  - `.shellcheckrc` no se lee.
  - Las columnas cuentan bytes, como el resto de las reglas; ShellCheck cuenta caracteres (la
    medición convierte).
  - El arnés de esta paridad (ShellCheck en `json1` contra kerbo en JSON) quedó fuera del repo, como
    los anteriores.
- Go contra golangci-lint (empezada, 2026-10-21): `ineffassign`, la única regla del conjunto por
  defecto de golangci-lint 2 (`standard`: errcheck, govet, ineffassign, staticcheck y unused) que no
  pide los tipos del compilador, con el nombre del linter, en una familia propia (`GoRule`).
  - Frecuencia del conjunto por defecto en gin, cobra, chi, caddy, prometheus, hugo, etcd y consul
    (golangci-lint 2.15, 2026-10-21), de mayor a menor: errcheck 5.647, staticcheck ST1005 844,
    SA1019 321, QF1008 316, unused 201, QF1012 58, govet inline 38, QF1004 32, govet stdmethods 28,
    ST1012 19 y el resto con menos de 15. ineffassign da 2: casi todos esos repos ya corren
    golangci-lint en su CI, así que lo que sobrevive es lo que ellos apagan (errcheck) o lo que
    trae la versión nueva de staticcheck. Para ineffassign la medición suma la biblioteca estándar
    de Go, terraform y golang/tools, que no usan golangci-lint.
  - Port 1:1 de ineffassign sobre un go/ast mínimo armado desde tree-sitter (`src/lint/go/ast/`),
    recorrido en el orden de campos de `ast.Walk`; la resolución de nombres de go/parser
    (`resolver.go`: alcances, `:=` que redeclara, etiquetas por función, claves de literales
    compuestos que se resuelven sin quedar pendientes, parámetros de tipo del receptor que se
    declaran sin ligarse y los nombres del paquete resueltos al final contra el mismo archivo), y
    el grafo de bloques de ineffassign con sus rarezas: un global declarado más abajo en el archivo
    cuenta como local, la clave de `point{x: 3}` se resuelve contra una variable local `x` y la
    usa, y un `defer` hace que cualquier operación que puede entrar en pánico lea los resultados con
    nombre.
  - Archivos generados como golangci-lint en modo `lax` (`code generated`, `do not edit`,
    `autogenerated file` o el encabezado de Swagger Codegen en los comentarios hasta la primera
    declaración después de `package`), y los que tree-sitter no parsea se saltean.
  - `//nolint` con el alcance de golangci-lint en el registro de `src/suppressions.rs`, así entra en
    la antitrampa con su motivo: el rango es el grupo de comentarios armado como go/parser (los del
    final de una línea van solos) y se estira hasta el final de cualquier nodo que empiece en la
    línea siguiente y en la misma columna; antes de `package`, vale para el archivo entero. Sin
    lista o con `all` vale para todas las reglas; con lista, para las que se llaman igual.
  - Paridad contra golangci-lint 2.15 (`--no-config --enable-only ineffassign
    --uniq-by-line=false`), restringida a los archivos que `go list` incluye para linux/amd64 con
    tests: cero diferencias por archivo, línea, columna y variable en la biblioteca estándar de Go
    (87), golang/tools (10), terraform (3), hugo (2), caddy (los 4 que silencia con `//nolint`),
    syncthing (1), consul, gin, cobra, chi, prometheus, etcd y fzf: 103 avisos en unos 10.500
    archivos. tailscale queda afuera: `go list` no lista sus archivos con el Go de la imagen.
  - Lo que no se ve a simple vista quedó en `src/lint/go/fixtures/`, con la salida de golangci-lint
    al lado.
- Pendientes de Go contra golangci-lint:
  - staticcheck ST1005 (844, mensajes de error con mayúscula o puntuación al final): reconoce
    `errors.New` y `fmt.Errorf` por el import, sin tipos; es la siguiente. Después, las de
    staticcheck que tampoco piden tipos: QF1004, QF1001 y QF1003.
  - errcheck, SA1019, QF1008, QF1012, unused y govet: piden los tipos del compilador. Es el grueso
    del volumen (errcheck solo es el 75 %) y hay que decidir cómo traerlos sin reimplementar el
    chequeo de tipos de Go.
  - Las restricciones de build: golangci-lint analiza sólo los archivos de la plataforma (por
    defecto linux/amd64) y kerbo, todos los `.go`; en un `_windows.go` kerbo puede marcar algo que
    golangci-lint no ve.
  - `uniq-by-line` (prendido por defecto en golangci-lint) deja un solo aviso por línea y linter;
    kerbo muestra todos.
  - `//nolint:funlen`, `//nolint:gocognit` y los demás linters que imitan las reglas universales
    no silencian `function-length`, `cognitive-complexity` y compañía: hoy sólo coinciden los
    nombres iguales.
  - El arnés de esta paridad (golangci-lint en JSON y `go list` contra kerbo en JSON) quedó fuera
    del repo, como los anteriores.
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
- Vulnerabilidades de dependencias con OSV y secretos versionados (hecho en main).
  - **`committed-secret`**: las 222 reglas de la configuración por defecto de gitleaks v8.30.1,
    copiadas tal cual en `src/secrets/gitleaks.toml` y leídas al arrancar, con sus palabras clave,
    allowlists (globales y por regla, con `condition`, `regexTarget`, rutas y stopwords),
    entropía, `secretGroup`, reglas sólo por nombre de archivo, `gitleaks:allow`, la
    decodificación de base64, hex, URL y escapes Unicode hasta 5 niveles, el descarte de binarios
    por firma y la precedencia de una regla específica sobre `generic-api-key` en la misma línea.
    Las regex de Go se traducen al dialecto de Rust (las llaves que Go toma literales) y se
    compilan cuando la palabra clave aparece. Mira todos los archivos, también los ocultos
    (`.env`, `.github/`), salvo `.git`, y respeta `.gitignore`. El secreto nunca sale entero: el
    reporte y la caché sólo guardan los primeros cuatro caracteres y el largo. Entra en la caché
    por contenido con la huella de qué patrones de ruta matchean, así el resultado no depende de
    la ruta (la caché pasa al formato 4 al sumar este tipo de entrada). Suma una regla propia, `connection-string-password` (`src/secrets/extra-rules.toml`),
    para contraseñas en URLs de bases, colas y FTP.
  - Paridad contra `gitleaks dir` (docker, configuración por defecto forzada con `--config` y sin
    `.gitleaksignore`), 2026-10-20, en 15 repos: gitleaks (con sus fixtures), trufflehog (miles de
    secretos falsos en los tests de sus detectores), Yelp/detect-secrets, OWASP juice-shop, flask,
    express, requests, fastapi, django, cpython, spring-boot, junit4, commons-lang, Guava y parte
    de VS Code: **2.251 hallazgos idénticos (archivo, línea y regla), 0 faltan y 0 sobran**. La
    regla propia suma 66, todos en fixtures de tests (65 de trufflehog, uno de spring-boot).
  - **`vulnerable-dependency`**, sólo con `kerbo check --osv` (la única regla con red): lee
    `Cargo.lock`, `package-lock.json`/`npm-shrinkwrap.json` (v1 a v3), `pnpm-lock.yaml` (v5 a v9),
    `yarn.lock` (clásico y berry), `go.mod` con `replace` y `go.sum` (sin repetir lo de `go.mod`),
    `poetry.lock`, `uv.lock`, `Pipfile.lock`, `requirements*.txt` fijados con `==`, `pom.xml` con
    los padres locales cuyas coordenadas coinciden, sus propiedades y su `dependencyManagement`, y
    `gradle.lockfile`. Consulta `api.osv.dev/v1/querybatch` (con paginado) y baja cada aviso;
    agrupa los que son alias entre sí en un solo problema con la versión que lo arregla. No usa
    un cliente HTTP propio: ureq con rustls no pasa `deny.toml` (webpki-roots tiene licencia
    CDLA-Permissive-2.0, y ring arrastra un segundo windows-sys), native-tls rompe el binario
    estático de musl, así que habla con OSV a través del `curl` del sistema, con 10 s para
    conectar, 60 s por pedido y los avisos en paralelo en un solo proceso. Las respuestas de
    `querybatch` quedan en la caché un día; los avisos, hasta que OSV informe otro `modified`.
    `kerbo osv-download [ecosistemas]` baja los `all.zip` de OSV (crates.io, npm, Go, PyPI y
    Maven: 280 MB, 31 s y 29 MB de memoria con el índice armado) y `--osv=offline` los lee sin red
    con un lector de zip propio (zip64 incluido) y miniz_oxide, evaluando rangos `SEMVER` y
    `ECOSYSTEM` y listas de versiones con comparadores de semver, PEP 440 y Maven.
  - Paridad contra osv-scanner 2.7.0 (docker), 2026-10-20: NodeGoat (package-lock), Vite 4.0
    (pnpm v5), Vue 2.6 (yarn), ripgrep 13 (Cargo), gin 1.7 (go.mod y go.sum), Poetry 1.1
    (poetry.lock), pydantic 2.10 (uv.lock) y djangoproject.com (uv.lock): **los 958 avisos que
    reporta osv-scanner, sin uno de más**; con la base bajada, 957 (el que falta OSV lo resuelve
    por un rango `GIT`). Los paquetes empaquetados dentro de otro (`inBundle`) no se reportan, como
    en osv-scanner. En WebGoat 8 (Maven, contra `--no-resolve`) están los 779 y sobran 12, todos
    reales: versiones que vienen de propiedades del POM raíz que osv-scanner no resuelve sin red.
  - Pendientes:
    - Maven sin resolución transitiva: osv-scanner por defecto le pide el árbol a deps.dev; kerbo
      sólo ve lo declarado en los POM locales (ni BOMs ni padres remotos como
      `spring-boot-starter-parent`). Gradle sólo con `gradle.lockfile`.
    - `requirements.txt` no sigue los `-r`/`-c` ni resuelve rangos; `go.mod` no suma la versión de
      `go`/`toolchain` como `stdlib` (osv-scanner tampoco lo hace hoy). Faltan `bun.lock`,
      `deno.lock`, `composer.lock`, `Gemfile.lock`, `packages.lock.json` y `pubspec.lock`.
    - Comparadores de versiones aproximados para el modo offline: PEP 440 sin segmentos locales y
      Maven sin las listas anidadas de `ComparableVersion`; los rangos `GIT` se ignoran.
    - Secretos sólo en el árbol actual: falta la historia de git (`gitleaks git`), donde queda lo
      que se borró después de commitear, y los archivos comprimidos (gitleaks tampoco los abre por
      defecto). Los archivos de más de 32 MB que no son código se saltean.
    - Las supresiones en el código valen para los secretos de los archivos de código; en los que
      no son código (`.env`, YAML, JSON) sólo vale `gitleaks:allow` o una excepción en `kerbo.toml`.
    - Memoria: los secretos suman las cachés por hilo de las regex que se usan (trufflehog pasa de
      120 a 250 MB; Guava, de 305 a 360 MB, con el mismo tiempo de CPU). Achicar la caché del DFA
      perezoso la baja un 30 % a cambio de un 40 % más de CPU; falta una estrategia mejor (regex
      por grupo de reglas o un prefiltro común).
    - La caché de OSV es un solo archivo: dos corridas a la vez pisan sus consultas (como la de
      análisis). Las respuestas viejas no se podan.
    - `kerbo baseline`, el hook y el servidor MCP todavía no consultan OSV; el hook y el LSP sí
      buscan secretos en el archivo editado.
    - Una regla de kerbo o una allowlist propia de secretos en `kerbo.toml`, además de las
      excepciones por ruta, y un `kerbo explain` por cada tipo de secreto.

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
    - `squawk-ignore-file` sin reglas (todas) no se reconoce: haría falta saber qué familia de
      reglas es de cada herramienta. `# shellcheck disable=all` y el alcance real de las
      directivas de ShellCheck quedaron hechos con las reglas `SC*`.
    - squawk asocia el comentario a la sentencia siguiente; kerbo, a la línea siguiente: un
      `ALTER TABLE` partido en varias líneas con la violación más abajo no queda cubierto.
    - SARIF no los manda como `suppressions` (`kind: inSource`, con `justification`); GitHub los
      mostraría como resueltos.
  - compose: el límite de CPU, `restart`, imágenes sin versión fija (como DL3006/DL3007) y los
    servicios que dependen de otro sin `condition: service_healthy`.

### 13. Formateo propio

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
