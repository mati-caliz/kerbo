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

### 3. Capa para agentes de IA (cerrada: lo central en 0.3, el servidor MCP en 0.4 y el arnés en main)

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
  - Lo que dijo el piloto del arnés de abajo (2026-10-21):
    - El agente lee las filas sin problema: las 12 corridas con kerbo terminaron en verde, sin
      perder ni confundir problemas.
    - El símbolo en las filas no se agrega: el agente igual lee el archivo entero antes de editar
      (Claude Code no deja editar sin leer), así que no ahorraría lecturas.
    - Los tokens se miden con el uso real que reporta la API en cada sesión, que es lo que se
      paga; contar la salida sola con el tokenizador de los Claude actuales ya no hace falta.
    - El corte de 30 problemas queda: con un archivo de 220 problemas, kerbo sale un 15 % más
      barato que ruff con la salida entera (tabla de abajo).
    - El agente no sabía que existe `kerbo fix`: con permisos para correr cualquier comando,
      arreglaba todo a mano mientras con ruff usaba `--fix`, porque ruff cierra su salida con
      cuántos arregla. Ahora la última línea de `--format agent` es `autofix[N]: kerbo fix` (y
      `autofix` en `agent-json`), contado sobre el reporte entero; el agente la usa.
- **El demonio para agentes** (hecho, 2026-10-21; el demonio en sí es de la fase 7):
  - El hook lo arranca solo: si nadie de este binario responde, corre el pedido en su proceso y
    después lanza `kerbo daemon run` desacoplado (si responde uno de otro binario, primero lo
    apaga). Un lock exclusivo sobre `daemon.lock` deja uno solo por carpeta de caché: si dos
    hooks lo lanzan a la vez, el segundo se va enseguida. `KERBO_DAEMON=off` lo deja afuera, y
    los tests de integración lo usan.
  - Atiende cada conexión en su hilo. Como el proceso tiene un solo directorio actual, los
    pedidos del mismo directorio corren en paralelo y los de otro esperan a que se vacíe (un
    `check` largo en un repo hace esperar al hook de otro repo, no al del mismo). Pasar el
    directorio a todo lo que hoy resuelve rutas relativas (escaneo, configuración, línea de
    base, git) sacaría también esa espera; no vale el cambio mientras cada sesión trabaje en un
    solo repo.
- **Cuánto gasta un agente de punta a punta** (piloto hecho, 2026-10-21; falta ampliarlo). Lo que
  paga quien lo usa con un agente: correr el gate, leer la salida, arreglar y volver a correr hasta
  que pase. El arnés está en `scripts/agent-harness/`: copia cada caso a una carpeta temporal con
  git, le pone un `gate.sh` (ruff 0.17 con `E4,E7,E9,F`, o golangci-lint 2.15 por Docker; o kerbo
  con `--format agent` y el hook de Claude Code) y corre `claude -p` con la misma instrucción,
  Sonnet y sólo `Read`, `Edit`, `Write`, `Glob`, `Grep` y `bash gate.sh`. Mide costo, tokens,
  vueltas, corridas del gate y tiempo de la sesión, vuelve a correr el gate al final y busca
  supresiones o archivos de configuración tocados en el diff. `--check` corre los dos gates sin
  agente, para confirmar que fallan por lo mismo (1 y 37 problemas en Python, 1 y 6 en Go).

  Piloto: Python y Go, un problema suelto y varios, tres corridas por caso (24 sesiones, 2,04 USD
  en total). Todas terminaron en verde y ninguna hizo trampa. Medianas (rango entre paréntesis):

  | Caso | Herramientas | Costo USD | Tokens de entrada | Vueltas | Corridas del gate | Segundos |
  |---|---|---|---|---|---|---|
  | Python, 37 problemas | ruff | 0,129 (0,119–0,130) | 265.473 | 13 (11–15) | 3 | 51 |
  | | kerbo | 0,114 (0,108–0,132) | 210.501 | 9 (9–13) | 3 | 39 |
  | Python, 1 problema | ruff | 0,053 (0,053–0,053) | 80.696 | 4 | 2 | 13 |
  | | kerbo | 0,062 (0,062–0,062) | 135.687 | 6 | 2 | 23 |
  | Go, 6 problemas | golangci-lint | 0,099 (0,093–0,100) | 144.294 | 11 | 2 | 41 |
  | | kerbo | 0,105 (0,099–0,105) | 205.036 | 13 (11–15) | 3 | 35 |
  | Go, 1 problema | golangci-lint | 0,060 (0,059–0,061) | 108.186 | 5 | 2 | 31 |
  | | kerbo | 0,059 (0,059–0,059) | 108.366 | 5 | 2 | 15 |

  Con muchos problemas en Python kerbo sale un 12 % más barato y más rápido; en Go es más
  rápido siempre (el gate tarda milisegundos y golangci-lint por Docker, segundos) y cuesta lo
  mismo. Las dos diferencias en contra salen de cómo trabaja el agente, según las trazas: con el
  parche de `--format agent` a la vista intenta arreglar con `sed` o un script de Python, el
  arnés se lo niega (sólo deja `bash gate.sh`) y rehace el cambio con `Edit`. Con un agente que
  puede correr cualquier comando eso no pasa, pero el arnés lo limita a propósito, igual para las
  dos herramientas. Los tokens de entrada son casi todos lecturas de la caché del modelo, por eso
  pesan poco en el costo.

  Ampliación (2026-10-21, 48 sesiones más, 4,82 USD): Java contra Checkstyle 14.3 (las seis
  reglas de forma), TypeScript contra ESLint 10.13 con typescript-eslint 8.72, Rust contra clippy
  con los umbrales de `strict` en `clippy.toml`, un archivo de Python con 220 problemas y una
  variante (`--wide`) en la que el agente puede correr cualquier comando. Todas en verde y sin
  trampas:

  | Caso | Herramientas | Costo USD | Vueltas | Corridas del gate | Segundos |
  |---|---|---|---|---|---|
  | Java, 28 problemas | Checkstyle | 0,118 (0,103–0,143) | 11 | 2 | 32 |
  | | kerbo | 0,107 (0,097–0,116) | 11 | 3 | 28 |
  | TypeScript, 19 problemas | ESLint | 0,100 (0,095–0,101) | 7 | 2 | 44 |
  | | kerbo | 0,103 (0,099–0,115) | 7 | 3 | 40 |
  | Rust, 4 problemas | clippy | 0,105 (0,104–0,112) | 6 | 2 | 27 |
  | | kerbo | 0,106 (0,105–0,107) | 8 | 2 | 41 |
  | Python, 220 problemas en un archivo | ruff | 0,175 (0,134–0,185) | 16 | 3 | 54 |
  | | kerbo | 0,148 (0,140–0,151) | 12 | 4 | 39 |
  | Con permisos amplios: Python, 37 | ruff | 0,083 (0,080–0,096) | 5 | 2 | 19 |
  | | kerbo, con `autofix` | 0,092 (0,075–0,098) | 6 | 4 | 24 |
  | Python, 220 | ruff | 0,071 (0,068–0,082) | 5 | 2 | 18 |
  | | kerbo, con `autofix` | 0,070 (0,069–0,072) | 6 | 3 | 16 |
  | TypeScript, 19 | ESLint | 0,079 (0,077–0,082) | 4 | 2 | 24 |
  | | kerbo, con `autofix` | 0,087 (0,084–0,089) | 4 | 3 | 22 |
  | Go, 6 | golangci-lint | 0,076 (0,070–0,077) | 5 | 3 | 36 |
  | | kerbo | 0,078 (0,078–0,086) | 4 | 2 | 24 |

  Con el agente limitado al gate, kerbo cuesta lo mismo o menos y es más rápido salvo en Rust
  (las vueltas de más son del agente, no del gate). Con permisos amplios cuesta lo mismo en
  archivos grandes y hasta un 10 % más en los chicos, dentro de la dispersión de las corridas.
  Lo que se esperaba ver (logs de Maven y Gradle, XML de SpotBugs, el mismo problema reportado
  dos veces) no aparece en casos así de chicos: las salidas de las herramientas de siempre son
  más largas, pero el agente las corta con `tail` y lo que paga es casi todo el contexto fijo de
  la sesión, leído de la caché del modelo. La ventaja de kerbo para agentes es la velocidad del
  gate y una sola herramienta para todos los lenguajes, no tokens.

  Quedó afuera el repo que se adopta con línea de base: ruff no tiene, golangci-lint usa
  `--new-from-rev` y Checkstyle un XML de supresiones, así que no hay una comparación pareja. El
  arnés se corre a mano antes de cada versión (unos 7 USD las dos tandas), así una regresión en el
  consumo se ve igual que una en la paridad.

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
  la etapa 8 y de ahí pasó a la 15.

### 5. Más reglas universales y Rust (cerrada: las reglas en 0.6, Rust en main)

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
- Paridad en Rust hasta donde llega sin compilar (hecho, 2026-10-21):
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
  - Después del expansor:
    - El hook reusa las definiciones (hecho, 2026-10-21): un escaneo de un solo archivo usa el
      índice compartido del proceso, el mismo del editor, que se revisa por fecha y tamaño. Con
      el demonio de la fase 7, el hook de un archivo de Rust de kerbo baja de 19 a 16 ms.
    - El costo (hecho, 2026-10-21). Perfilado con perf: casi todo era volver a parsear con
      tree-sitter los argumentos de cada invocación, que miran la complejidad, el anidamiento y
      la búsqueda de closures, y otra vez desde cada argumento que contiene a una invocación
      anidada. Ahora cada argumento se parsea una vez (por lugar y texto, así que lo guardado es
      lo mismo que daría parsear de nuevo), sólo se buscan closures en invocaciones con un `|`, y
      cada archivo reusa las definiciones que ya leyó el índice del crate en vez de leerlas otra
      vez. Con el mismo reporte en siete crates: winnow de 247 a 219 ms (sin macros, 134),
      serde_json de 167 a 137 (sin macros, 112), clap_builder de 121 a 114. Lo que queda es
      medir el código expandido, que es lo que hace falta para la paridad.
    - La huella del crate en la clave de la caché (hecho, 2026-10-21): la lista de archivos y el
      texto sólo de los que tienen una definición, un `#[macro_use]` o una invocación de una macro
      que arma funciones, que es lo único que cambia el análisis de otro archivo. Editar una
      función común ya no invalida el resto del crate.
  - **Macros de otros crates** (hecho, 2026-10-21). Las `macro_rules!` exportadas por las
    dependencias del `Cargo.toml` (normales, de desarrollo, de build y por plataforma, también las
    de `path`), leídas de la caché de cargo con la versión del `Cargo.lock` o, sin lockfile, la
    más nueva compatible con el requisito (`^`, como cargo). Se cargan recién cuando una invocación
    las nombra: `dep::macro!`, `$crate::...` adentro de una macro de esa dependencia, o un nombre
    suelto que trae `use dep::...`, `use dep::*` o `#[macro_use] extern crate dep`. Lo que arman no
    se marca en la dependencia (clippy no reporta adentro de macros externas), pero suma a la
    complejidad de quien la invoca. tree-sitter-rust lee `str![...]` (la macro de snapbox) como un
    índice con un error: para parsear se le cambia el nombre por otro del mismo largo y el texto
    original sigue mandando. Las macros procedurales (`derive`, `#[tokio::main]`,
    `automod::dir!`) quedan afuera: hay que compilarlas.

    Paridad contra clippy en los ocho crates (con el arnés de `ideas/kerbo/parity`, rutas
    normalizadas): de 544 diferencias a 64. winnow de 402 a 18 (los `str!` de snapbox),
    toml_parser de 36 a 0 e indexmap de 54 a 0 (`quickcheck!`). Lo que queda: serde_json, 32 por
    un `#[cfg]` adentro de una función y 10 por un módulo que declara una macro procedural; el
    resto, sueltas. Medir lo que ahora se expande cuesta: winnow tarda de 194 a 270 ms; los demás
    casi no cambian. Corriendo el arnés apareció que kerbo tomaba `dockerfile.rs` por un
    Dockerfile: ahora la extensión de un lenguaje manda sobre el nombre.
  - **`#[cfg]` evaluado** (hecho, 2026-10-21). Un ítem, una sentencia o un brazo de `match` con un
    `#[cfg]` falso no cuenta para la complejidad ni para el anidamiento (el largo sigue contando el
    texto, como `too_many_lines`), y un archivo con `#![cfg(..)]` falso no se mide. Se evalúa con
    lógica de tres valores: la máquina donde corre kerbo, las features del `Cargo.toml` (todas,
    como `--all-features`) y `test` en los dos modos; lo desconocido (un `cfg` del `build.rs`, un
    `target_feature`) no decide y el código se mide, porque esconderlo por adivinar sería peor.
    `[rust]` en `kerbo.toml` cambia `target_os`, `target_arch`, las `features` y los `cfg`
    encendidos, y entra en la clave de la caché.

    Paridad contra clippy en los ocho crates y kerbo, con el filtro de `cfg` del arnés: de 64
    diferencias a 28, y seis crates en cero (kerbo incluido). serde_json baja de 30 a 0 en la
    complejidad. Sin el filtro del arnés, kerbo solo deja 45, todas de funciones bajo un `cfg` del
    `build.rs` que kerbo mide a propósito.
  - Lo que queda no se puede sin compilar, y ahí la referencia sigue siendo clippy: el `Result` que
    devuelve un closure sin anotarlo (16 en winnow), los módulos que declara una macro procedural
    (10 en serde_json, `automod::dir!`) y código escrito adentro de una macro (2). Los dos casos
    del expansor sin cubrir (una función armada por una macro con el `fn` de la definición y los
    extremos del usuario, y macros no compartidas que invocan funciones-macro de otro archivo) no
    aparecen en ninguno de los nueve crates.
  - El arnés de paridad de Rust está en `ideas/kerbo/parity` (`run_rust.sh`, `cfg_filter.py`,
    `rust_macro_diffs.py`), como el resto. El crate con un caso por criterio se perdió, pero sus
    casos viven como tests de kerbo (`src/metrics/clippy/expansion_tests.rs` y los de
    `src/macro_rules`), con los valores de clippy fijados.

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
  - El deploy del sitio (hecho, 2026-10-21): publicaba una copia local vieja del repo, así que
    `en/`, `styles.css`, `copy.js` e `install.ps1` daban 404. Ahora un script del servidor (en el
    repo de infraestructura) saca `site/` de `origin/main` y la sincroniza entera, y nginx sirve
    `install.sh` e `install.ps1` como `text/plain; charset=utf-8`.
  - Plurales en español (hecho: el resumen, la línea de base, el hook y el aviso de tests
    borrados dicen "1 archivo", "1 problema" y "1 desvío").
  - El error "tree-sitter no devolvió un árbol" y el nombre de las funciones sin nombre (hecho:
    los dos pasan por el catálogo; el identificador estable sigue usando "(anónima)", así las
    líneas de base existentes no cambian, y un test lo fija).
  - `README.en.md` y `site/en/` se mantienen a mano (hecho el chequeo: `tests/translations.rs`
    compara los títulos, las tablas y los bloques de código de los dos README, y las secciones,
    filas, bloques de instalación y comandos copiables de los dos sitios; si algo está en un
    idioma y no en el otro, el gate falla).

### 7. Caché y rendimiento (cerrada)

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
    Releer, combinar y renombrar va adentro de un lock exclusivo del sistema operativo sobre un
    archivo `.lock` por shard (`File::lock`: `flock` en Unix, `LockFileEx` en Windows), así que
    no queda ventana; si el proceso muere, el sistema lo suelta solo.
  - El tope configurable (hecho: `max_megabytes` en `[cache]` de `kerbo.toml` y
    `KERBO_CACHE_MAX_MB`, que manda; 256 MB de fábrica). La caché es una por usuario y la comparten
    los proyectos, así que el tope que vale es el del último que escribe: es a propósito, porque
    un tope por proyecto obligaría a llevar la cuenta de qué entrada es de quién, y la misma
    entrada sirve a todos los clones.
  - La memoria en caliente de Guava (hecho, 2026-10-21). Era casi toda tokens de CPD
    recombinados enteros. Recombinar sólo los grupos que tocan lo que cambió no da lo mismo que
    CPD: recorre los grupos en el orden de un `HashMap` de Java cuya capacidad sale del total de
    tokens, y ese orden decide qué coincidencia queda cuando dos se pisan. Lo que sí se achicó es
    cada token: la línea en 32 bits, y en el pool la posición en 12 bytes (antes un `Option` de
    cuatro campos de 64 bits), la imagen una sola vez por texto distinto y el índice del hash en
    32 bits; de unos 84 bytes por token a unos 40. En Guava, con el mismo reporte, el pico del
    contenedor en caliente baja de 253 a 204 MB y en frío de 362 a 319 MB (los dos incluyen la
    caché de páginas de leer el repo).
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
  Con `MSYS_NO_PATHCONV=1`, `pwd` devolvía `/c/Users/...` y Docker Desktop a veces montaba `/src`
  vacío según la carpeta: el estándar (`quality/rust/verify-rust.sh` y `quality/go/verify-go.sh`)
  ahora monta con `pwd -W` cuando existe (Git Bash) y con `pwd` en el resto (hecho, 2026-10-21).
- Una sola pasada por función para todas las métricas (medido y descartado, 2026-10-21). En Go,
  sin caché y en 4 núcleos, apagar todas las métricas por función baja la CPU de 74 a 65 s en la
  biblioteca estándar, de 30 a 27 s en consul y de 5,2 a 4,8 s en hugo, y el tiempo real casi no
  se mueve: lo domina leer y parsear. Juntar las pasadas ahorraría como mucho la mitad de ese
  10 %, y obliga a fundir ciclomática y cognitiva, que están ajustadas una por una contra cada
  herramienta de referencia.
- Un demonio por máquina (hecho, 2026-10-21): `kerbo daemon start|status|stop`. `kerbo check` y
  el hook le mandan los argumentos, el directorio, el idioma y la entrada por TCP en `127.0.0.1`
  (un solo camino para Linux, macOS y Windows; la std no trae named pipes), con el puerto y un
  token en `daemon.json` dentro de la caché, sólo legible por el dueño. El demonio corre el mismo
  `app::run` con la caché ya cargada en memoria y devuelve la salida y el código de salida. Si no
  hay demonio, si es de otro binario o con otro tope de caché, o si algo falla, el comando corre
  solo como antes, con la entrada ya leída. Se apaga solo tras 30 minutos sin uso. Atiende de a
  un pedido por vez, porque cambia el directorio del proceso: un `check` largo hace esperar al
  hook. El MCP y el LSP ya son procesos largos y siguen corriendo en el suyo, con la caché
  compartida dentro del proceso.
  En un contenedor con 4 núcleos, sobre kerbo mismo: el hook pasa de 20 a 14 ms con un archivo de
  Python mínimo y de 29 a 20 ms con uno de Rust, y `kerbo check src` en caliente de 29 a 23 ms,
  con el mismo reporte. Lo que queda es arrancar el proceso cliente (unos 13 ms). Lo que falta
  del demonio quedó en la fase 3.

### 8. Java sintáctico (cerrada)

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
- La familia `JavaRule` (2026-10-21): las reglas de `sun_checks.xml` que siguen entran como una
  familia (`rule_family!`, como Python y Go), así cada una es una entrada y su texto. Primera
  tanda: `TypeName`, `MethodName`, `ConstantName`, `StaticVariableName`, `MemberName`,
  `ParameterName`, `UpperEll` (con arreglo) y `EmptyStatement`. Paridad con el arnés de forma, por
  archivo, línea, columna y regla: cero diferencias sobre commons-lang, junit4 y Guava (9.800
  avisos, 7.868 de ellos `MethodName` en Guava por los métodos de test con guiones bajos).
  `minimal` las apaga. Lo fijado en `src/lint/fixtures/Names.java`: `MethodName` saltea
  `@Override` y marca el método con el nombre de la clase; un campo de interfaz es constante;
  `serialVersionUID` no; los patrones son ASCII.
- Segunda tanda (2026-10-21): `ModifierOrder`, `ArrayTypeStyle`, `MultipleVariableDeclarations`,
  `InterfaceIsType`, `MissingSwitchDefault`, `NeedBraces`, `EmptyBlock`, `AvoidNestedBlocks`,
  `InnerAssignment`, `SimplifyBooleanExpression`, `EqualsHashCode`, `HideUtilityClassConstructor`,
  `TodoComment`, `IllegalImport`, `FileTabCharacter`, los espacios al final (`RegexpSingleline`) y
  `NewlineAtEndOfFile` (las dos últimas con arreglo). Cero diferencias sobre los tres repos en las
  31 reglas de forma (unos 53.000 avisos). Lo que hubo que calcar, fijado en
  `src/lint/fixtures/Shapes.java`: una anotación después de los modificadores vale si va sobre el
  tipo (variables, parámetros, constructores y métodos que devuelven algo), y un comentario entre
  los modificadores no cuenta; `EmptyBlock` no mira el `catch`; `MissingSwitchDefault` no cuenta
  el `default` de un switch anidado y saltea los que usan patrones o `case null`; `InnerAssignment`
  acepta el idioma del `while` también combinado con `&&` o `||`; `SimplifyBooleanExpression` marca
  un ternario sólo si las dos ramas son literales; `EqualsHashCode` mira cualquier cuerpo de clase
  y no cuenta un `hashCode` abstracto; `HideUtilityClassConstructor` descarta las clases que
  heredan, las abstractas y las anidadas static, cuenta los campos de instancia y toma como público
  el constructor de paquete; `TodoComment` marca dos columnas después del inicio del comentario.
- Tercera tanda (2026-10-21): `DesignForExtension`, `HiddenField`, `VisibilityModifier`,
  `DeclarationOrder` y `LocalFinalVariableName`. Cero diferencias sobre los tres repos (unos 30.000
  avisos; 18.378 son `DesignForExtension` en Guava). Lo calcado, fijado en
  `src/lint/fixtures/Design.java`: `DesignForExtension` mira la visibilidad de las clases y no la
  del método, encuentra el Javadoc aunque haya comentarios en el medio pero no lo ve en un método
  sin modificadores que arranca con `<T>`, y a un método sin modificadores lo ubica en el tipo de
  retorno (el último punto de un nombre calificado); `HiddenField` toma como static las interfaces
  y los records pero no los enums, y en una clase anónima el contexto lo da el campo o bloque que
  la contiene; `VisibilityModifier` acepta `@Rule` y compañía si están importadas o son del mismo
  paquete; `DeclarationOrder` corre también en interfaces (un campo sin `static` cuenta como de
  instancia), no marca la visibilidad de un campo cuyo valor usa uno anterior o a sí mismo, y una
  visibilidad fuera de orden no cambia la esperada. Las cinco reglas se calculan recorriendo el
  árbol de arriba hacia abajo: subir al padre en tree-sitter es caro. Velocidad en Guava (3.275
  archivos, 4 CPU, archivos dentro del contenedor): kerbo entero 10,7 s y ~350 MB sin caché,
  0,7 s con caché; Checkstyle con sólo las reglas de forma, 34,9 s y ~1,3 GB.
- Cuarta tanda, espacios y llaves (2026-10-21): `WhitespaceAround`, `WhitespaceAfter`,
  `NoWhitespaceBefore`, `NoWhitespaceAfter`, `GenericWhitespace`, `MethodParamPad`,
  `EmptyForIteratorPad`, `LeftCurly`, `RightCurly` y `OperatorWrap`. Sobre los tres repos, 8.672
  avisos y una sola diferencia: `ClassUtilsOssFuzzTest` de commons-lang tiene bytes NUL dentro de
  un texto, tree-sitter corta el literal ahí y toma un `+` de adentro como operador. Como en los
  repos casi no aparecen `RightCurly`, `WhitespaceAfter` ni `NoWhitespaceBefore`,
  `src/lint/fixtures/Spacing.java` junta los casos y coincide con Checkstyle en sus 59 avisos.
  Los siete de espacios traen arreglo (agregar o sacar el blanco en la misma línea); las llaves y
  `OperatorWrap` no, porque piden reacomodar líneas: eso es del formateador de la etapa 13. Lo
  calcado:
  - `WhitespaceAround` con las opciones de `sun_checks` no perdona bloques vacíos (`{}` se marca
    dos veces); un `}` seguido de `)`, `,`, `;` o `.` y un `return` sin valor no piden blanco
    después; la llave de un bloque de inicialización pegada a la de su clase (`{{`, `}}`) tampoco.
    Los dos puntos de un ternario y de un `assert` sí, los de un for mejorado no.
  - `NoWhitespaceBefore` no permite cortes de línea: una coma o un `;` al principio de una línea
    también se marcan, salvo el `;` que cierra una parte vacía de un `for`.
  - `NoWhitespaceAfter` mira en el `[` de un arreglo o un índice el blanco entre lo anterior y él,
    y en un inicializador de arreglo la llave que abre.
  - `GenericWhitespace` pide blanco antes del `<` sólo en los parámetros de tipo de un método o
    constructor; después del último `>` acepta blanco, `)`, `,`, `[`, `.`, `:` o `;`, y en una
    llamada (`Foo.<T>bar()`, `new Foo<T>()`) nada en blanco; uno anidado admite un solo blanco y
    sólo antes de un `&`.
  - `EmptyForIteratorPad` marca lo primero que sigue al blanco, no el blanco.
  - `LeftCurly` mira que no haya código después de la llave sólo en los bloques (no en los
    cuerpos de los tipos ni del switch), y no mira los bloques sueltos ni los de inicialización.
  - `RightCurly` ubica la sentencia anterior a la llave por su `;` si es una expresión o una
    declaración (en el árbol de Checkstyle el `;` va aparte) y por su principio si no.
  - `OperatorWrap` no cuenta los paréntesis que envuelven un operando.
  - Lo que tree-sitter no pudo parsear (un ERROR) no se mira, salvo el `...` con una anotación
    antes, que tree-sitter deja en tres puntos sueltos y se reconoce para `NoWhitespaceBefore`.
  - Velocidad en Guava con las diez (mismas condiciones que la tanda anterior): 8,9 s y ~375 MB
    sin caché, 0,55 s con caché; Checkstyle con todas las reglas de forma, 31,5 s y ~1,5 GB. Las
    reglas de espacios recorren las hojas del árbol una vez, con el padre anotado al bajar.
- Pasan a la etapa 15 la paridad contra PMD y Qodana, las reglas de Javadoc y las inspecciones de
  la API nueva; `Indentation` y `LineLength` van con el formateo de la etapa 13.
- Las reglas de forma que no entran, por frecuencia en `sun_checks.xml` entero sobre los mismos
  cuatro repos (todas las demás ya están):
  - `Indentation` (696.473) y `LineLength` (228.941): son de formato y van con la etapa 13; sin
    medirlas contra el formateador del proyecto marcarían casi todo Guava, que usa
    google-java-format.
  - Javadoc (`JavadocVariable`, `MissingJavadocMethod`, `JavadocMethod`, `JavadocType`,
    `JavadocPackage`, `InvalidJavadocPosition`): pasan a la etapa 15.
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
  - Lo que le faltaba a `kerbo fix` (hecho, 2026-10-21):
    - El "arreglar todo" del LSP hace las mismas vueltas que el CLI sobre el texto del editor y
      manda una sola edición que reemplaza el documento: el protocolo pide ediciones sobre el
      texto original, y así lo que destapa una vuelta entra sin pedirlo de nuevo.
    - Una vuelta que deja más nodos de error o faltantes de tree-sitter que el texto original no
      se acepta (el análisis se recupera y no lo notaría).
    - El parche conserva el `\r` de cada línea y marca `\ No newline at end of file`, así
      `git apply --unidiff-zero` lo aplica también con CRLF. Sigue sin contexto, a propósito.
    - `--since <referencia>` arregla sólo los problemas de las líneas que cambiaron, como
      `check --diff`; las vueltas siguientes se limitan a esos mismos problemas (por su huella).
      No se llama `--diff` porque ese es el alias de `--dry-run`, como en ruff.
  - El arnés de esta paridad (hecho, 2026-10-21): `run_java_shape.sh`, `checkstyle.shape.xml` y
    `shape_compare.py` en `ideas/kerbo/parity`, que compara por archivo, línea, columna y regla y
    pasa las columnas de Checkstyle a bytes. Una regla nueva se suma con una entrada en
    `SHAPE_RULES`. Sobre commons-lang, junit4 y Guava: cero diferencias en 41.400 avisos.

### 9. Inspecciones de IntelliJ propias y flujo de datos sin compilar (cerrada)

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
  - Java hacia SQL, comandos y rutas (hecho, 2026-10-22): `sql-injection`, `command-injection` y
    `path-traversal` (`src/lint/java_taint/`, `src/index/java_taint.rs`). Un intérprete por
    método con constantes (para decidir `if`, `switch` y ternarios), listas por posición y mapas por
    clave, y un resumen por método que el índice cruza hasta un punto fijo: qué devuelve según sus
    parámetros y qué parámetros reciben algo del usuario desde alguna llamada.
  - Medido contra el OWASP Benchmark 1.2 (la verdad de cada caso, no otra herramienta): en las tres
    categorías detecta todos los casos vulnerables (272, 126 y 133) y ningún falso positivo (de 232,
    125 y 135). Semgrep 1.140 con `p/java` sobre los mismos casos: 86 %, 89 % y 90 % detectados
    con 62 %, 77 % y 79 % de falsos positivos (puntaje +24 %, +12 % y +12 %).
  - Lo que hizo falta para llegar a cero: un tipo del archivo de quien llama tapa a los homónimos
    (cada caso tiene su clase interna `Test`) y lo que devuelve un método abstracto de una
    interfaz que nadie implementa (Spring Data) no arrastra sus argumentos: sin eso, en gastronova
    una entidad leída por id parecía armada por el usuario.
  - En los tres servicios, Guava, commons-lang y junit4: ningún aviso. Sin fuentes en el proyecto
    (una librería), el cruce no corre: Guava sigue en 0,9 s con caché y 6,8 s sin caché.
  - Java hacia HTML (hecho, 2026-10-22): `xss`. Cada flujo lleva aparte lo que pasó por un escape
    de HTML, que no cuenta para un XSS pero sí para una consulta o una ruta, y cada método guarda
    qué parámetros reciben algo crudo y cuáles algo escapado. Los sumideros son los métodos de
    escritura de lo que devuelve `response.getWriter()` o `getOutputStream()`, encadenado o en una
    variable. Contra el Benchmark: 246 de 246 y ningún falso positivo de 209; Semgrep, 82 % con
    52 % de falsos positivos (+30 %). En los repos reales, ningún aviso.
- Java contra los tests de las reglas de taint de Semgrep (2026-10-22): la primera medición daba 6
  avisos "de más". Tres eran falsos positivos y se corrigieron: un parámetro del pedido numérico,
  booleano, `UUID` o de fecha no puede llevar texto (`@RequestBody Integer`), `FilenameUtils.getName`
  y `getBaseName` limpian una ruta, y `exec(new String[] {"programa", x})` sin shell sólo cuenta si
  el usuario elige el programa (como en Python y JS; la lista de shells pasa a `src/lint/taint/
  commands.rs`, compartida; los arreglos `new T[] {...}` y `{...}` se siguen por posición). Los otros
  tres son desacuerdos: dos `exec(args, entorno)` con el entorno del usuario, que el propio test llama
  "vulnerable, pero otro tema", y un literal con un escape inválido (`\l`) que no compila. Detecta 6
  de 9 comandos, 2 de 2 rutas, 7 de 83 SQL y 2 de 13 XSS: lo que falta son las reglas `audit` de
  Semgrep (`spring-sqli` toma cualquier parámetro de un `@RequestMapping` como fuente, y
  `formatted-sql-string` cualquier texto armado con `format`). El Benchmark sigue en 100 % sin
  falsos positivos y en los servicios y librerías no hay avisos.
- Pendientes de contaminación:
  - Un `PrintWriter` que llega como parámetro o campo no se reconoce como la respuesta; tampoco las
    plantillas (Thymeleaf con `th:utext`, JSP con `<%= %>`) ni `@ResponseBody` que devuelve HTML.
  - FindSecBugs como segunda referencia: pide compilar el Benchmark con Maven.
  - Los campos no se siguen: un dato del usuario guardado en `this.x` y leído en otro método se
    pierde.
  - Lambdas y clases anónimas (hecho, 2026-10-22): corren en el lugar con lo que capturan. Los
    parámetros de la lambda de `forEach`, `map`, `filter`, `ifPresent`, `thenApply` y parecidos valen
    lo que trae el receptor, y lo que devuelve sólo sale como resultado en los métodos que lo usan
    así (`map`, `reduce`, `orElseGet`, `computeIfAbsent`...). Con `orElseThrow` no: la primera
    versión daba 2 falsos en gastronova porque el mensaje de la excepción, armado con el id del
    pedido, contaminaba la entidad que devolvía el repositorio. En el Benchmark sigue en 100 % sin
    falsos positivos; en los tres servicios, Guava, commons-lang y junit4, ningún aviso.
  - Los sumideros con receptor desconocido (una cadena `a.b().query(...)`) no se miran.
  - Sumideros de MyBatis (`${}` en los mapeos XML y en `@Select`).
- Contaminación en Python (hecho, 2026-10-22): las cuatro reglas pasan a una familia propia
  (`TaintRule`, los mismos nombres) y el motor queda compartido (`src/lint/taint/`,
  `src/index/taint_engine.rs`); cada dato lleva un bit por tipo de sumidero para el que sigue siendo
  peligroso, así un escape de HTML, un chequeo de `../` o un `shlex.quote` limpian sólo lo suyo.
  Python tiene su intérprete (`src/lint/python/taint/`), con constantes, `match`, `elif`, `with`,
  listas, diccionarios y `ConfigParser` por clave, tuplas, f-strings y `split` plegado, y su
  resolución entre módulos (`src/index/python_taint.rs`): el nombre de módulo sale de la ruta, y
  con un `src/` adelante se busca por sufijo.
  - Contra el OWASP Benchmark for Python 0.1: las cuatro categorías detectan todo (5, 13, 65 y 31)
    y ningún falso positivo (de 11, 7, 103 y 58). Semgrep 1.140 con `p/python`: 0 %, 46 % con
    100 % de falsos, 3 % y 0 %.
  - Contra los tests de las reglas de taint de Semgrep para Python (las líneas `# ruleid:` y
    `# ok:`, sin las reglas cuya fuente es `os.environ` o `sys.argv`): ningún aviso de más;
    detecta 39 de 71 comandos, 14 de 16 SQL y 16 de 29 XSS. Lo que falta, explicado: las reglas de
    Semgrep no coinciden entre sí sobre `subprocess` sin shell (la de Django lo cuenta como
    inyección, la de Lambda como seguro), y kerbo sólo lo cuenta con shell; el XSS de Lambda
    (devolver HTML desde un handler) no se mira.
  - En tus proyectos con Python (finarg, propmetrics, soma y comex-ai): ningún aviso. La primera
    versión daba 84, todos falsos: el `return` de una ruta de FastAPI es JSON, no HTML, y las
    sentencias de SQLAlchemy Core van con parámetros ligados.
- Pendientes de contaminación en Python:
  - Fuentes de línea de comandos (`sys.argv`, `os.environ`, `input()`), apagadas para no marcar
    scripts y herramientas.
  - El código de nivel de módulo (fuera de funciones) no se recorre.
  - La herencia entre clases y los métodos de clases que llegan como parámetro no se resuelven.
- Contaminación en JavaScript y TypeScript (hecho, 2026-10-22): un intérprete
  (`src/lint/ecmascript/taint/`) con constantes, `switch`, plantillas de texto, objetos y arreglos
  por clave y posición (`push`/`pop`/`shift`), y la resolución entre archivos por `import` y
  `require` (`src/index/ecmascript_taint.rs`: el módulo sale de la ruta sin extensión ni `/index`).
  Como en las demás reglas de proyecto, sólo corre si el análisis abarca el proyecto entero.
  - Contra los tests de las reglas de taint de Semgrep para JavaScript: 16 de 21 SQL y 18 de 33
    XSS. Los 6 "de más" son desacuerdos: uno es una inyección real marcada `ok` porque es de otra
    regla, y cuatro son `res.send("message: " + x)` de Express, que manda `text/html` (Semgrep sólo
    avisa si el texto tiene una etiqueta). Lo que falta, explicado: las reglas `audit` de Semgrep
    toman cualquier parámetro como fuente y `path.join` como sumidero (comandos 1 de 8, rutas 0 de
    13, y en TypeScript las de MCP y `DomSanitizer` de Angular); algunas marcan la línea donde se
    arma el texto y kerbo la del sumidero.
  - En tus proyectos con JS/TS (gastronova, chirola, lantix, atlas, respondi, ritmo, conseguilo,
    halo, portfolio, bender, finarg, propmetrics, soma y tradeagent): ningún aviso. La primera
    versión daba 13, todos falsos: `reply.send` de Fastify (texto plano o JSON), respuestas que
    declaran JSON con `writeHead` y un `escapeHtml` propio.
- Código de nivel de módulo en JavaScript y TypeScript (hecho, 2026-10-22): lo que corre al cargar
  el archivo (un script del navegador, `export const ...`, las rutas que se registran fuera de una
  función) se recorre como una función más, `<módulo>`, así un `document.write(location.hash)`
  suelto avisa. En los tests de Semgrep, lo mismo; en tus proyectos, ningún aviso.
- Pendientes de contaminación en JavaScript y TypeScript:
  - Sin tipos, las conexiones y los objetos del pedido se reconocen por el nombre de la variable;
    con el chequeo de tipos de TypeScript se podría mirar el tipo.
- Plantillas en archivos, Python y JavaScript (hecho, 2026-10-22; estaba a medias en un stash y se
  terminó): `render_template` de Flask, `render` y `render_to_response` de Django y `res.render` de
  Express dejan un sumidero por cada variable que le pasan a la plantilla, con su nombre. Si alguien
  renderiza una, el índice lee las plantillas del proyecto (`.html`, `.jinja`, `.j2`, `.ejs`, `.hbs`,
  `.mustache`, `.pug`, sin `node_modules`; si nadie renderiza, no camina el disco) y sólo avisa si esa
  variable sale sin escapar: `{{ x|safe }}` y `{% autoescape false %}` en Jinja, `<%- x %>` en EJS,
  `{{{ x }}}` en Handlebars y Mustache, `!{x}` en Pug. La plantilla se busca por ruta, por sufijo o
  por nombre sin extensión (`res.render('pagina')`).
  - Medido: los benchmarks de Java y Python siguen en 100 % sin falsos positivos, los tests de Semgrep
    de JS y Python dan lo mismo y en tus proyectos no hay avisos. Ninguno de esos corpus tiene
    plantillas que el arnés cuente (las de Semgrep son `.html` sueltos), así que lo nuevo sólo está
    probado con los tests unitarios: falta un caso real.
  - Faltan Thymeleaf (`th:utext`) y JSP (`<%= %>`) en Java, y `html/template` vs `text/template` de
    archivo en Go.
- Clausuras en JavaScript y TypeScript (hecho, 2026-10-22): una función escrita dentro de otra (el
  callback de `.then`, `.map`, `.forEach` o de una ruta registrada dentro de una función) corre en
  el lugar con las variables de afuera. Los parámetros de los callbacks de `.then`, `.map`,
  `.filter`, `.reduce` y parecidos valen lo que trae el receptor; los de cualquier otro, nada
  conocido. Lo que el callback les asigna a las variables de afuera se junta con lo que tenían (puede
  no haber corrido), lo que devuelve es el valor del argumento, y un `res.type('json')` adentro sólo
  vale para ese callback. En los tests de Semgrep y en tus proyectos con JS/TS, lo mismo que antes
  (ninguno de los que faltan es por clausuras); el tiempo sobre gastronova sin caché no cambia.
- Contaminación en Go (hecho, 2026-10-22): un intérprete (`src/lint/go/taint/`) que usa los tipos
  declarados para las fuentes y los sumideros, con constantes, `switch`, `range`, mapas por clave y
  `strings.Builder`, y la resolución entre paquetes por carpeta (`src/index/go_taint.rs`). Los
  archivos con contaminación llevan una sola lista en el análisis y su familia sale del lenguaje.
  Un sumidero dentro de un bucle avisa una sola vez (antes, uno por vuelta).
  - Contra los tests de las reglas de taint de Semgrep para Go: detecta todo (10 SQL, 2 rutas y 4
    XSS; tres caen unas líneas más abajo, en el sumidero, y no en donde se arma el texto). Dos "de
    más" son desacuerdos: un texto que Semgrep no cuenta como SQL porque no empieza con una palabra
    de SQL (`FAKE DELETE ...`), y `filepath.Join(raiz, strings.Trim(r.URL.Path, "/"))` sin `Clean`,
    que es un recorrido de carpetas salvo que el router limpie la ruta.
  - En tus proyectos con Go (atlas, despe, dotfiles y soma): ningún aviso. La primera versión daba
    uno, falso: el gateway de dotfiles reenvía la respuesta de otro servidor con su `Content-Type`.
- Campos de struct en Go (hecho, 2026-10-22): el tipo de `h.store` sale del struct declarado en el
  archivo, así `h.store.Query(...)` con `store *sql.DB` es un sumidero y `h.db.Query(...)` con `db`
  un caché propio ya no lo es (antes se decidía por el nombre del campo). Los structs de otro
  archivo del paquete todavía no se ven: ahí sigue decidiendo el nombre. En los tests de Semgrep, los
  mismos avisos; en tus proyectos con Go, ninguno.
- Funciones anónimas en Go (hecho, 2026-10-22): como las clausuras de JavaScript, la de un
  `HandleFunc`, un `go func() { ... }()` o una que se guarda en una variable corre en el lugar con
  lo que captura; un parámetro `*http.Request` trae datos del usuario, lo que les asigna a las
  variables de afuera se junta con lo que tenían y un `Content-Type` que no es HTML sólo vale para
  esa función. En los tests de Semgrep para Go, los mismos avisos que antes; en tus proyectos con
  Go, ninguno.
- Contaminación, segunda tanda (hecho, 2026-10-22):
  - Fuentes de JAX-RS, Quarkus y Micronaut (`@QueryParam`, `@PathParam`, `@RestQuery`,
    `@QueryValue`...), de Nest (`@Query()`, `@Body()`...), de Hono (`c.req.query()`) y de Next
    (`request.nextUrl`, las `searchParams` de una página).
  - Sumideros de jOOQ con SQL en texto (`dsl.fetch(...)`, `DSL.sql`, `DSL.field`), `c.Data(...,
    "text/html", ...)` de gin, `Markup`/`mark_safe`/`jinja2.Template` en Python y `text/template`
    (que no escapa) ejecutado sobre la respuesta en Go.
  - Inyección de argumentos sin shell, en los cuatro lenguajes: el usuario elige el programa
    (`exec.Command(prog)`, `spawn(cmd)`, `subprocess.run([cmd, ...])`). Un texto suelto sin shell
    en Python no cuenta: es el nombre del programa entero.
  - La respuesta HTML de un handler de Lambda (`{body, headers}`), también en Python, y los escapes
    propios por nombre quedan en `src/lint/taint/responses.rs`, compartidos.
  - Medido: el OWASP Benchmark de Java y el de Python siguen en 100 % sin falsos positivos; en los
    tests de Semgrep, los comandos de Python pasan de 39 a 41 de 71 y los de JavaScript de 1 a 2 de
    8. El único aviso "de más" nuevo es correcto: `subprocess.call([cmd[0], ...])` con `cmd` del
    usuario, que Semgrep da por bueno porque no hay shell. En tus proyectos, ninguno.
- **Renombres de columnas y tablas que el código desplegado todavía usa** (viene de la etapa 12).
  Sin compilar no hay forma confiable: los nombres pasan por los mapeos del ORM (`@map` de
  Prisma, `name=` de SQLAlchemy, los nombres de drizzle), cambian de caso y coinciden con palabras
  comunes (`name`, `status`), así que buscar el nombre viejo en el código da falsos positivos y
  negativos. Con el índice del proyecto se puede leer el esquema del ORM y cruzarlo con la
  migración; mientras tanto, como squawk, todo renombre se marca.
  - Revisado (2026-10-22) con los 3 renombres de `V1__initial_schema.sql` de gastronova: por texto,
    `ingredientName` aparece en 18 archivos Java que son DTOs y excepciones sin relación con la
    columna, y `rated_at` en 3 con consultas de otras tablas. Además, el código que rompe un
    renombre es el desplegado, la versión de antes del cambio; el de la revisión actual suele venir
    ya renombrado junto con la migración. Hacerlo bien pide dos cosas que todavía no hay: indexar la
    revisión base en `--diff` (el resumen de cada archivo de esa revisión, de la caché por
    contenido) y leer el mapeo de JPA (`@Table` y `@Column(name = ...)`, o el nombre de la
    clase y del campo en snake_case como Spring) para saber qué entidad usa cada tabla y columna.
    Queda para la etapa de despliegue, junto con Prisma, SQLAlchemy y drizzle.
- Paridad contra Qodana con el perfil `Default`, inspección por inspección. Donde Qodana se
  equivoca por no conocer el framework, la diferencia queda fijada en un test como mejora, no
  como desvío.
- `SameParameterValue` para lo privado (hecho, 2026-10-21): `same-parameter-value`, en la
  familia `JavaRule`. El 89 % de lo que marca Qodana (198 de 222 en los tres servicios) son
  métodos privados, y para ésos alcanza el archivo: todas sus llamadas están en su clase de nivel
  superior. Así no hace falta el índice, entra en la caché por archivo y funciona igual en el LSP
  y en el hook.
  - Lo que se calcó de IntelliJ: con una sola llamada alcanza; cuentan los constructores privados,
    los de un enum (las constantes con argumentos los llaman) y el de un record privado; el valor
    se muestra como literal tal cual o como `Clase.CONSTANTE` (un campo static final, uno de una
    interfaz o una constante de un enum), y `-1` cuenta como constante.
  - Para no adivinar: si hay otro método con el mismo nombre y la misma cantidad de parámetros en
    la clase, si se lo usa como referencia o si es varargs, no se marca. Una llamada calificada
    con otro objeto cuenta igual, aunque vaya a otra clase: sin tipos no se sabe, y contarla sólo
    puede hacer que falte un aviso, nunca que sobre.
  - Paridad contra los SARIF de Qodana de los tres servicios, sólo en lo privado: 197
    coincidencias por archivo, línea y columna, ningún aviso de más y uno de menos
    (`.product(flan)` de un builder con el mismo nombre que un método privado del test). Los
    valores mostrados coinciden con los de IntelliJ. En Guava, commons-lang y junit4 da 102 avisos;
    los revisados a mano son correctos.
- El índice del proyecto, primera versión (hecho, 2026-10-21): cada archivo Java deja un resumen
  (`src/index/java.rs`: métodos y constructores con visibilidad, `@Override`, aridad, si tienen
  cuerpo y qué devuelven; llamadas con sus argumentos constantes y si usan el resultado;
  referencias a métodos; tipos, paquete y constantes con su visibilidad), que va a la caché por
  contenido con el resto del análisis. El scan junta los resúmenes y corre las reglas de proyecto
  (`src/index/java_rules.rs`) sólo si analiza la raíz: con una carpeta o un archivo suelto faltan
  llamadas. Todavía sin resolver tipos: una llamada va a todo método con su nombre y aridad, y si
  hay más de uno declarado en el proyecto no se razona sobre ninguno.
  - `SameParameterValue` de paquete y protegido. Como IntelliJ, el valor tiene que poder llevarse
    adentro del método: un literal, una constante del mismo archivo, una pública o de afuera del
    proyecto, o una de paquete del mismo paquete (y no de los tests, si el método no lo es).
  - `UnusedReturnValue` (`unused-return-value`): un método que devuelve algo y al que todas las
    llamadas le tiran el resultado. Lo que da un `case X -> metodo();` de un switch que devuelve
    valor se usa.
  - `SameReturnValue` (`same-return-value`): todos los `return` devuelven el mismo literal (una
    constante con nombre no: es la forma de exponerla). Alcanza con que ningún método con el
    mismo nombre y aridad diga `@Override`; lo privado, con que no haya otro igual en su clase.
  - Contra los SARIF de Qodana de los tres servicios: `SameParameterValue` 215 coincidencias y 7
    faltantes, `UnusedReturnValue` 13 y 6, `SameReturnValue` 14 y 4 (los 4 son "todas las
    implementaciones de una interfaz devuelven lo mismo", que pide jerarquía de tipos). Ningún
    aviso de más en las tres. Los faltantes son sobrecargas o nombres repetidos en el proyecto,
    que piden resolver tipos.
  - Costo en Guava, proyecto entero: 10,3 s y ~430 MB sin caché (unos 55 MB más), 0,73 s con
    caché (la pasada del índice corre igual: es barata).
- `unused` para métodos (hecho, 2026-10-21): `unused-method`. El `unused` de Qodana Community es
  alcanzabilidad desde los puntos de entrada sin conocer Spring: de sus 3.919 avisos, en métodos
  728 llevan anotaciones de framework y 231 son accessors, y los campos "sin usar" o "nunca
  asignados" son de JPA o inyectados. kerbo da por usado lo que nombra cualquier llamada,
  referencia o texto del proyecto, y tiene los frameworks como tablas de datos: las anotaciones
  pasivas (con cualquier otra el método se da por llamado por un framework), los tipos cuyos
  métodos llama el framework (`@Mapper`, `@Aspect`, endpoints), los accessors y los nombres que
  llama el JDK. Lo público y protegido sólo en una aplicación (`@SpringBootApplication` y
  similares): junit4 trae `main` en sus runners y no por eso es una aplicación.
  - Paridad: como Qodana marca de más, la meta es que todo lo de kerbo esté también en Qodana. En
    los tres servicios, 13 avisos, todos coinciden. Lo que falta son consultas de repositorios y
    helpers de tests cuyo nombre se usa en otra clase: piden resolver tipos.
  - En Guava, commons-lang y junit4 (librerías: sólo lo no público): 50, 3 y 45 avisos, los
    revisados son helpers sin uso o métodos que se buscan por reflexión recorriendo todos los de
    la clase (que IntelliJ también marca).
  - Faltan campos, constructores y clases sin usar: piden distinguir lectura de escritura y las
    tablas de JPA, Jackson y la inyección (un campo `@Autowired` o `@Column` se usa aunque nadie lo
    lea en el código).
- Tipos de los receptores y jerarquía (hecho, 2026-10-21): `src/lint/java_receivers.rs` resuelve
  sin compilar el tipo donde empieza a buscarse cada llamada (una variable, parámetro o campo
  declarado en el archivo con un solo tipo; `Clase.metodo()`; `metodo()` y `this.metodo()` en las
  clases que la encierran, contando una anónima como el tipo que crea), y el resumen guarda los
  padres de cada tipo. Una llamada va a un método si su clase es el tipo del receptor o uno de sus
  padres; las de receptor desconocido (cadenas, lambdas sin tipos, `import static`, variables de
  tipo genérico) siguen yendo a todos. Dos métodos iguales ya no se descartan por estar en
  cualquier parte del proyecto, sólo si están en la misma clase o en clases emparentadas; si hay
  otros iguales sin relación, hace falta al menos una llamada que se sepa que va a éste.
  - Paridad contra Qodana, sin avisos de más: `SameParameterValue` 217 coincidencias y 5
    faltantes (antes 215 y 7), `UnusedReturnValue` 15 y 4 (antes 13 y 6), `unused-method` 19
    (antes 13).
  - En una librería, lo público y lo protegido es API: `unused-return-value` y
    `same-parameter-value` no lo miran (commons-lang marcaba 16 retornos de API pública).
- Firmas de las dependencias, primera parte (hecho, 2026-10-21): `src/jvm/`. Un lector de zip
  propio (directorio central, entradas comprimidas con `miniz_oxide`, zip64) que lee del disco
  sólo el índice y las clases pedidas, y un lector de `.class` sin bytecode: padres, anotaciones
  de clase (las de `package-info` también) y, por método, las de cada parámetro, incluidas las de
  tipo (el `@Nullable` de JSpecify es `TYPE_USE` y va en otra tabla). El classpath se le pide a
  Maven offline una vez y se guarda en la caché con la huella de los `pom.xml`;
  `KERBO_JAVA_CLASSPATH` lo da a mano (Gradle y el resto). Sólo se carga si una regla lo usa.
  - Gradle propio (hecho, 2026-10-22): sin `pom.xml`, con `build.gradle`, `settings.gradle` (o sus
    `.kts`), `gradle.lockfile` o `libs.versions.toml`, se le pide a Gradle (el wrapper del proyecto
    si está, con `sh` porque el checkout puede no tener el permiso de ejecución) con un script de
    inicio propio, `--offline` y sin la caché de configuración. La vista tolerante de los artefactos
    saltea lo que no está bajado: en soma, `runtimeClasspath` pide `spring-boot-devtools`
    (`developmentOnly`), que nunca se bajó, y sin eso fallaba todo. En soma da 290 jars y los mismos
    avisos que con el classpath a mano.
  - Todavía sin el JDK desde `ct.sym`: trae el API hasta la versión anterior a la del JDK (la
    actual está en `lib/modules`, en formato jimage), y lo que hace falta del JDK para las reglas de
    hoy (qué se cierra, qué puede ser null) no está en las firmas sino en tablas propias.
- `NullableProblems` para parámetros (hecho, 2026-10-21): `nullable-problems`. Los padres de la
  clase se resuelven con los imports del archivo (explícitos, con comodín, el mismo paquete,
  `java.lang`) y se sube por el proyecto y por la librería hasta el método redefinido; si su
  clase, una que la encierra o su paquete son `@NullMarked` o `@NonNullApi`, cada parámetro sin
  anotación que en la librería no es `@Nullable` se marca. Como IntelliJ, el parámetro que el
  cuerpo desreferencia en todos los caminos (`p.algo()` antes de cualquier salida temprana; de
  un `if` cuenta la condición) se infiere no nulo y no se marca (`src/lint/java_inference.rs`).
  - La variante del retorno (hecho): un método que devuelve un objeto, redefiniendo uno cuyo
    retorno no es `@Nullable`, sin anotación propia. IntelliJ no infiere nada para el retorno:
    marca aunque devuelva un literal o un `new`. El lector de `.class` suma las anotaciones del
    método y las de tipo con destino "retorno". Una clase anónima redefine los métodos del tipo
    que crea, y un varargs cuenta como parámetro.
  - La inferencia no mira adentro de un `try` (sólo sus recursos): IntelliJ tampoco.
  - Contra Qodana, con los jars que usó Qodana (los volúmenes de su caché): 84 coincidencias,
    ningún aviso de más (antes de la inferencia eran 23 de más) y 2 faltantes: anotaciones
    `@NotNull` que IntelliJ le agrega al JDK por fuera de los jars.
- Nulos, primera parte (hecho, 2026-10-21):
  - `data-flow-issue`: en una cadena de llamadas, un paso que la librería declara `@Nullable`
    seguido de otra llamada sobre su resultado. Cada archivo guarda sus cadenas (raíz: una
    variable con su tipo declarado o una clase; pasos con nombre, aridad y posición) y el índice
    las recorre con los jars: el tipo de cada paso es la clase que devuelve el anterior según su
    firma; con sobrecargas del mismo nombre y aridad, sólo si todas dicen lo mismo. Un resultado
    ya chequeado antes en el archivo (por texto: `!= null`, `assertThat(...).isNotNull()`,
    `assertNotNull`, `requireNonNull`) no se marca, como IntelliJ.
  - Se probó seguir una variable inicializada con una llamada (`Cache cache = m.getCache(x);
    cache.clear();`) y dio 31 avisos de más: el código real chequea con `if (x == null)
    return;` o `x == null || x.algo()`, y eso pide flujo de datos de verdad, que queda para la
    parte que sigue.
  - Contra Qodana: 12 coincidencias, ningún aviso de más. Lo que falta son las otras variantes
    (unboxing, argumento que puede ser null, variables, `Map.get` y lo del JDK, cuyas anotaciones
    IntelliJ trae aparte).
  - `optional-get-without-is-present` (por archivo, sin librerías): `get()` encadenado a lo que
    crea el `Optional`. Las 3 de Qodana, exactas.
- Nulos, segunda parte: variables (hecho, 2026-10-21). `src/lint/java_null_flow.rs` recorre cada
  método como un intérprete abstracto sobre el árbol (sin grafo explícito: Java es estructurado):
  el estado son las variables que podrían ser null y la asignación que lo causó; `if`, `&&`,
  `||`, `!`, ternarios, `instanceof`, `== null` con salida, `hasText`/`isNull`, `requireNonNull`
  y `assertThat(x).isNotNull()` parten el estado por camino; los caminos que salen (`return`,
  `throw`, `break`) no se juntan; el `finally` arranca también desde la entrada del `try`. Como la
  nulabilidad de la llamada sólo se sabe con los jars, cada archivo guarda por variable la cadena
  que le dio valor y los usos sin chequeo, y el índice los reporta si esa cadena termina en algo
  `@Nullable` (también cuando la raíz es una clase del proyecto que hereda el método de una
  librería, y a través de un cast). Los tipos de las variables se miran por método: el mismo
  nombre puede ser de otro tipo en otro método. Los chequeos por texto valen dentro del método.
  - Contra Qodana: 17 coincidencias, ningún aviso de más.
- Recursos sin cerrar (hecho, 2026-10-21): `auto-closeable-resource`. Cada método deja sus
  candidatos (`src/lint/java_resources.rs`): una llamada que inicializa una variable local que
  nunca va a un `try (variable)`, no se devuelve (fuera de una lambda), no se asigna, no recibe
  `close()` ni se pasa como argumento (fuera de una lambda); y una llamada sobre cuyo resultado se
  llama a otra. El índice decide si lo que devuelve se cierra, siguiendo la cadena por el proyecto
  (el tipo que declara cada método, resuelto con los imports de su archivo), los jars (subiendo
  hasta `AutoCloseable`) o la tabla del JDK (`src/index/java_jdk.rs`: qué tipos se cierran, cuáles
  IntelliJ no pide cerrar, y qué devuelven `Executors.new*`, `HttpClient.newHttpClient`,
  `Files.new*`, `getConnection`...). Lo que devuelve un `get...` no cuenta como creado.
  - Contra Qodana: 9 de 10 coincidencias. Falta `dataSource.unwrap(HikariDataSource.class)`
    (devuelve un genérico) y sobra uno: un `ExecutorService` de `AuditTrailIntegrationTest` igual
    en forma a otros dos del mismo archivo que Qodana sí marca; no se encontró qué lo distingue.
- Campos sin usar (hecho, 2026-10-22): `unused-field`. Cada archivo guarda sus campos (con las
  anotaciones propias y las de la clase) y los identificadores que usa fuera de las declaraciones de
  campos. Un campo privado se da por usado si su archivo lo nombra; uno de paquete, protegido o
  público, si lo nombra cualquier archivo (lo público y protegido sólo en una aplicación). Los
  frameworks, como tablas en `src/index/java_unused.rs`: la inyección (`@Autowired`, `@Inject`,
  `@Value`…) no salva un campo que nadie lee, las demás anotaciones no pasivas sí, y las anotaciones
  de clase que no leen campos de instancia (estereotipos de Spring, tests, constructores de Lombok)
  no los salvan. El texto que inicializa a una variable de su mismo nombre (`X = "X"`) ya no cuenta
  como mención, tampoco para `unused-method`. Los campos de un `@interface` son públicos, como los
  de una interfaz (antes `visibility()` los tomaba como de paquete).
  - Contra Qodana: 53 coincidencias y 1 de más, que es una mejora: un `private final AuthService` de
    un `@Service` con `@RequiredArgsConstructor` que nadie usa (Qodana no conoce Spring y da la
    clase entera por inalcanzable, así que no mira sus campos). De los 376 que faltan, 369 son
    `@MockitoBean`, `@Mock` y `@Spy` en tests (reemplazan un bean o se inyectan en lo que se prueba:
    se usan), 4 son `@Column` de una entidad, 2 son `@Autowired` cuyo nombre usa otra clase y 1 es
    una constante con el mismo nombre que otra que sí se usa.
  - En Guava y commons-lang (librerías: sólo lo no público): 64 y 42. Son constantes sin uso, los
    campos de relleno de `Striped64` contra el false sharing y, en commons-lang, campos que los
    tests leen por reflexión recorriendo todos los de la clase. IntelliJ también los marca.
- Más código muerto (hecho, 2026-10-22):
  - `unread-field`: un campo privado que su archivo sólo escribe ("Field is assigned but never
    accessed" de Qodana). Cada archivo guarda los nombres que sólo aparecen como destino de una
    asignación simple; un parámetro o una variable local del método o la lambda más cercana tapa al
    campo del mismo nombre, sin mirar adentro de las clases anidadas. Contra Qodana: el único caso
    real (`InvoiceType.adjustmentNote` en gastronova) coincide y no sobra nada; los otros 5 son
    `@Column`, que lee JPA. En Guava, 4 (los tres revisados son campos asignados que nadie lee); en
    commons-lang, 18, casi todos de tests que leen por reflexión los `*Builder`.
  - Un `@interface` de nivel superior entra al resumen como los anidados.
- Pendientes de código muerto:
  - Resolver el dueño de cada uso de un campo (como los receptores de las llamadas): hoy un nombre
    usado en cualquier clase salva a todos los campos que se llaman igual.
  - "Asignado pero nunca leído" para lo que no es privado: pide juntar los nombres sólo escritos de
    todo el proyecto.
  - Constructores y constantes de enum sin uso: casi todo lo que marca Qodana es inyección de Spring,
    records que llegan por JSON y valores que manda el frontend; hace falta saber qué tipos
    deserializa Jackson antes de marcar algo.
  - Clases sin usar: con la jerarquía y las mismas tablas de frameworks.
  - Revisado (2026-10-22): en los tres servicios no hay ningún caso real de tipo, constructor o
    constante de enum sin usar. Los dos "Enum is never used" de Qodana (`RowOutcome` en comex,
    `PrintDocumentType` en gastronova) son enums privados que se usan desde métodos que Qodana da
    por inalcanzables; los constructores son de beans de Spring y de records que llegan por JSON.
    Sin un caso para medir, estas reglas no se hacen todavía.
- Nulos, tercera parte (hecho, 2026-10-22):
  - Tabla propia del JDK (`src/index/java_jdk.rs`): qué devuelven y si pueden ser null
    `getClass`, `getResourceAsStream`, `getResource`, `System.getProperty`/`getenv` y los de `File`.
    Anda sin classpath. `Path.getFileName` y compañía quedaron afuera: IntelliJ no los marca (había
    uno de más en comex).
  - El flujo de nulos sigue las variables de un `try` con recursos.
  - Unboxing: una cadena (también de un solo paso) cuyo resultado `@Nullable` se devuelve en un
    método primitivo o inicializa una variable primitiva, en el principio de la expresión.
  - Sobrecargas: el lector de `.class` guarda el bit de varargs, y un argumento `X.class` deja sólo
    las sobrecargas que reciben un `Class` en esa posición (`queryForObject(sql, Long.class, ...)`
    contra `queryForObject(sql, rowMapper, ...)`, que en Spring 7 no es `@Nullable`).
  - Contra Qodana: comex pasa de 3 a 8 coincidencias (los 4 unboxing y el `getResourceAsStream`),
    sin avisos de más en los tres servicios.
  - Los jars de los servicios se rehicieron desde cero (Maven `dependency:go-offline` y un script de
    Gradle que pide los archivos de `testRuntimeClasspath`) y dan exactamente lo de antes.
- Lo que sigue en nulos: el tipo de retorno de los métodos del proyecto (para seguir cadenas que
  pasan por ellos), argumentos que pueden ser null hacia parámetros no nulos (también los que
  IntelliJ infiere no nulos porque el método los desreferencia), lo que devuelve una lambda
  (`timer.record(() -> ...)` de Micrometer es `@Nullable`) y `Map.get`.
  - Revisado (2026-10-22): de los 3 "Passing 'null' argument to parameter annotated as non-null"
    de Qodana, dos pasan por `Map.get` sobre un genérico (sin tipos genéricos no se sigue) y uno es
    un `null` dentro de un `assertThatThrownBy` hacia un parámetro que el método desreferencia: pide
    calcular la inferencia de no nulo para todos los métodos (hoy sólo para los `@Override`) y
    guardar la posición de cada argumento `null`.
- "Todas las implementaciones devuelven lo mismo" (hecho, 2026-10-22): `same-return-value` sobre un
  método abstracto, con la jerarquía del índice y las clases anónimas. Los 4 de Qodana en
  gastronova son proyecciones de Spring Data con stubs sólo en los tests: hace falta una
  implementación de producción, así que kerbo no los marca, a propósito. Una interfaz con un solo
  método abstracto se salta (puede tener lambdas). En los servicios y las librerías no aparece
  ninguno: queda fijado en un test.
- `ClassEscapesItsScope` (hecho, 2026-10-22): `class-escapes-its-scope`, el "tipo privado expuesto"
  de arriba. Cada archivo guarda sus tipos con la visibilidad que tienen de verdad (la menor entre la
  suya y la de los que los contienen) y los tipos escritos en la firma de cada miembro no privado
  (lo que devuelve, sus parámetros, el tipo de un campo, los componentes de un record, también
  dentro de `List<...>`), con la visibilidad efectiva del miembro. El índice resuelve cada nombre
  contra los tipos anidados del archivo o los de nivel superior del mismo paquete; un parámetro de
  tipo o un import con el mismo nombre lo tapan, y si dos tipos se llaman igual no se marca.
  - Contra Qodana: 33 de 33 por archivo, línea y columna, sin avisos de más. En Guava 85,
    commons-lang 5 y junit4 17; los revisados a mano son tipos privados en firmas de paquete,
    protegidas o públicas.
- Parámetros sin usar (hecho, 2026-10-22): `unused-parameter`, el "Parameter is not used" del
  `unused` de IntelliJ. Un parámetro que el cuerpo nunca nombra, también en un handler de Spring
  (`@PathVariable String slug` que nadie lee). No se marca si el método redefine a otro o es
  redefinido, si hay uno igual en una clase emparentada (implementa algo sin `@Override`), si la
  clase hereda algo de afuera del proyecto y el método no es privado (puede implementar uno de una
  librería), en un `main`, en `readObject`/`writeObject` ni en un `@ParameterizedTest`; los
  componentes de un record son campos y siempre cuentan como usados.
  - Contra Qodana: 27 de 27, sin avisos de más (falta la variante "not used in any implementation"
    de un método de interfaz, 1 caso). En Guava 52, commons-lang 37 y junit4 80, casi todos métodos
    de prueba para reflexión.
- `BooleanMethodIsAlwaysInverted` (hecho, 2026-10-22): `boolean-method-is-always-inverted`. Cada
  llamada del resumen guarda si su resultado se descarta, se usa o se usa negado (`!activo()`, también
  entre paréntesis). Avisa en un método que devuelve un primitivo (un `!` sólo va delante de un
  boolean), con cuerpo, que no redefine ni es redefinido, y al que todas las llamadas del proyecto
  niegan, con al menos dos: con una sola IntelliJ no lo marca. Con homónimos en clases sin relación
  sólo cuentan las llamadas de receptor resuelto. Contra Qodana: 7 de 8, sin avisos de más; el que
  falta es un `matches` con homónimos sin receptor resuelto.
- `FieldMayBeFinal` (hecho, 2026-10-22): `field-may-be-final`, por archivo (un campo privado sólo se
  escribe desde su tipo de nivel superior). Un campo privado, no `final` ni `volatile`, que se
  inicializa en la declaración y no se vuelve a escribir, o (si no es estático) que cada constructor
  asigna una vez con una sentencia suelta o delega con `this(...)`, y nadie más lo escribe. No se
  marca con una anotación que lo escribe por reflexión (`@Autowired`, `@Value`, `@Mock`, `@Spy`,
  `@InjectMocks`, `@TempDir`...) ni en una clase con `@Data` o `@Setter`. Contra Qodana: 12 de 12,
  sin avisos de más. `CanBeFinal` (14) marca los mismos campos más los dos `@Spy` de gastronova, que
  esa inspección global no excluye: no se calca aparte.
- Cierre (2026-10-22). Contra Qodana con el perfil `Default` en los tres servicios (62 inspecciones,
  5.479 avisos), kerbo calca sin avisos de más: `SameParameterValue`, `UnusedReturnValue`,
  `SameReturnValue`, el `unused` de métodos, campos y parámetros, `NullableProblems`, `DataFlowIssue`,
  `OptionalGetWithoutIsPresent`, `AutoCloseableResource`, `ClassEscapesItsScope`,
  `BooleanMethodIsAlwaysInverted` y `FieldMayBeFinal`. Lo que quedó abierto se repartió así:
  - A la etapa 16 (tipos y firmas completas), porque pide infraestructura nueva: `RedundantThrows`
    (el atributo `Exceptions` de las firmas de las librerías), el JDK desde `ct.sym`, `Map.get` y lo
    que devuelve una lambda en nulos (tipos genéricos), el tipo de retorno de los métodos del
    proyecto y los argumentos null hacia parámetros no nulos (la inferencia para todos los métodos),
    y en JS/TS reconocer conexiones y pedidos por el tipo de TypeScript en vez del nombre.
  - A "cuando aparezca un caso real" (abajo), porque sin un caso no hay paridad que medir.
  - A la etapa 15 lo de modernización y redundancias de Qodana (`SequencedCollectionMethodCanBeUsed`,
    `Java9CollectionFactory`, `DefaultAnnotationParam`, `UnnecessaryUnicodeEscape`, `MathClamp`,
    bloques de texto, `RedundantTypeArguments`, `Convert2MethodRef` y compañía).
  - Fuera de alcance: las de Lombok (`LombokGetterMayBeUsed`, `LombokSetterMayBeUsed`), las de XML,
    Maven y Gradle, `ExtractMethodRecommender` y `CanBeFinal` (los mismos campos que
    `FieldMayBeFinal` más los que escribe Mockito).
  - Los renombres de columnas y tablas, a la etapa de despliegue (ya estaba dicho arriba).
- Cuando aparezca un caso real (sin uno en los servicios, las librerías ni los corpus, no se hace):
  - Contaminación en Java: un `PrintWriter` que llega como parámetro o campo, Thymeleaf
    (`th:utext`) y JSP (`<%= %>`), datos guardados en `this.x` y leídos en otro método, cadenas con
    receptor desconocido (`a.b().query(...)`) y MyBatis (`${}`); FindSecBugs como segunda referencia.
  - Contaminación en Python: la herencia entre clases y los métodos de clases que llegan como
    parámetro. Las fuentes de línea de comandos siguen apagadas a propósito, y con ellas no hace falta
    recorrer el nivel de módulo.
  - Contaminación en Go: `html/template` contra `text/template` cuando la plantilla está en un archivo.
  - Código muerto: el dueño de cada uso de un campo, "asignado pero nunca leído" para lo no privado,
    y constructores, constantes de enum y clases sin usar.
  - La variante "Parameter is not used in any implementation" de un método de interfaz.

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
- Python contra el conjunto por defecto de ruff 0.17 (empezado, 2026-10-23). Frecuencia en los diez
  repos de arriba, sin el conjunto clásico: UP031 15.546, I001 6.736, RUF059 4.549, C408 4.413,
  UP045 4.371, RUF012 4.039, UP006 4.025, UP032 1.920, B018 1.692, UP025 1.634, SIM117 1.535, RUF100
  1.437, UP007 1.410, BLE001 1.367, UP035 1.169, UP004 995, ISC004 947, SIM102 708, SIM115 633,
  PIE790 587, B023 583, UP005 540, C405 532, TRY002 517 y el resto con menos de 500.
  - Hechas: UP031 (el formato con `%`, con el parser de especificadores de F501–F509 parte por parte
    del string y el `clean_params_dictionary` de pyupgrade para decidir si avisa en el string o en la
    operación) y C408 (`dict()`, `list()` y `tuple()` del builtin sin posicionales). Paridad: 15.540
    de 15.546 y 4.385 de 4.413, sin avisos de más; lo que falta está en carpetas ocultas (`.ci`,
    `.azure-pipelines`), que kerbo no recorre, y en los archivos de CPython que tree-sitter no
    parsea. Los casos quedaron en `src/lint/python/fixtures/modern.py`.
  - Hechas en una segunda tanda: UP025 (prefijo `u`), UP004 (heredar de `object`), UP005 (alias de
    `unittest`), UP009 (declaración de codificación utf-8), C405 (`set([...])`), B015 (comparación
    suelta), B018 (expresión suelta, con el `contains_effect` de ruff; una t-string no cuenta como
    string) y PIE790 (`pass` o `...` de más, salvo `...` en `if TYPE_CHECKING:`, protocolos y métodos
    abstractos). Paridad: 1.634 de 1.634, 975 de 995, 540 de 540, 396 de 397, 532 de 532, 249 de 249,
    1.667 de 1.692 y 587 de 587, sin avisos de más; lo que falta está en los archivos de CPython que
    tree-sitter no parsea y en carpetas ocultas. Los casos quedaron en `fixtures/upgrades.py`.
  - Hechas en una tercera tanda: UP006 (`List`, `Dict` y compañía de `typing`, en cualquier lectura y
    también adentro de anotaciones entre comillas), UP045 (`Optional[X]`) y UP007 (`Union[X, Y]`),
    con las excepciones de ruff: `NamedTuple` adentro, un elemento con `*`, y un elemento entre
    comillas cuando la expresión se evalúa en ejecución. El contexto de tipos sale de los ancestros:
    `if TYPE_CHECKING:`, o una anotación que Python no evalúa (con el import del futuro, o la de una
    variable local o `self.x` dentro de una función). La versión de Python por defecto de ruff 0.17 es
    la 3.11, así que las tres valen en todo el código. Paridad: 4.006 de 4.025, 1.404 de 1.410 y 4.371
    de 4.371, sin avisos de más; lo que falta está en CPython sin parsear y en carpetas ocultas. Los
    casos quedaron en `fixtures/pep604.py`.
  - Hechas en una cuarta tanda: SIM102 (`if` anidados que se juntan con `and`, salvo chequeos de
    `sys.version_info`/`sys.platform` con cualquier alias, `TYPE_CHECKING` y `__main__`), SIM117
    (`with` anidados, salvo los de timeout de asyncio, anyio y trio) y BLE001 (`except Exception`
    sin `raise` ni registro con `logging.exception` o `exc_info=`, con la heurística de loggers de
    ruff). Paridad: 708 de 708, 1.535 de 1.535 y 1.304 de 1.367, sin avisos de más; los 63 que faltan
    están en archivos de CPython con `lazy import`. Los casos quedaron en `fixtures/simplify.py`.
  - Hechas en una quinta tanda: TRY002 (`raise Exception`), PLC0414 (`import x as x`, fuera de stubs y
    de `__init__.py`), S102 (`exec`), S110 (`except` amplio con sólo `pass`), SIM118 (`in d.keys()`,
    en comparaciones, `for` y comprensiones), UP017 (`datetime.timezone.utc`), UP018 (`str("x")`,
    `int(1)` y compañía) y UP030 (`"{0} {1}".format`). Paridad: 516 de 517, 459 de 460, 417 de 450,
    251 de 265, 335 de 335, 340 de 343, 213 de 213 y 489 de 489, sin avisos de más; todo lo que falta
    está en archivos de CPython que tree-sitter no parsea. Los casos quedaron en `fixtures/hygiene.py`.
  - Hechas en una sexta tanda: UP008 (`super(Clase, self)`, con las clases anidadas, `__class__`,
    los nombres ligados en la función y `dataclass(slots=True)`), PIE808 (`range(0, n)`), B009 y
    B010 (`getattr`/`setattr` con nombre fijo), RUF015 (`list(x)[0]` y `.pop(0)`), B006 (defaults
    mutables) y B008 (llamadas en defaults). B006 y B008 usan un port de los helpers de mutabilidad
    de ruff (`semantic/mutability.rs`: tipos de retorno mutables e inmutables, anotaciones
    inmutables con `Optional`, `Union`, `|` y `Annotated`, y `NewType` sobre un tipo inmutable), que
    también va a servir para RUF012. Paridad: 423 de 449, 244 de 245, 181 de 185, 82 de 84, 417 de
    420, 214 de 221 y 348 de 351, sin avisos de más. Lo que falta está en archivos que tree-sitter no
    parsea, salvo un B009 con un identificador que lleva un selector de variación Unicode (la
    biblioteca estándar de Rust no expone XID; haría falta `unicode-ident`). Los casos quedaron en
    `fixtures/defaults.py`.
  - Hecha RUF012 (atributos de clase mutables): corre al final del recorrido, como los scopes
    diferidos de ruff. Los nombres se resuelven desde el scope donde está la clase (ruff usa la
    resolución que guardó al visitar cada nombre) y las bases heredadas desde el módulo (ruff usa
    `lookup_attribute` desde el scope global). Respeta `ClassVar` y `Final` (sólo si el archivo
    importa `typing`), las anotaciones inmutables, dataclasses y attrs, los modelos de pydantic,
    msgspec y sqlmodel (también heredados de una clase del archivo) y el `_fields_` de ctypes.
    Paridad: 4.009 de 4.039, sin avisos de más; lo que falta está en archivos que tree-sitter no
    parsea. Los casos quedaron en `fixtures/classes.py`.
  - Hecha RUF059 (variable desempaquetada sin usar): son los mismos candidatos que F841, con los
    bindings desempaquetados que F841 descarta. Paridad: 4.511 de 4.549, sin avisos de más; lo que
    falta está en archivos que tree-sitter no parsea.
  - Hechas SIM115 (abrir un archivo fuera de un `with`, con las excepciones de ruff: `.close()`
    inmediato, `return`, `ExitStack`/`AsyncExitStack`, los contextos de unittest y `__enter__`) y
    TRY004 (chequeo de tipo con `isinstance`, `issubclass` o `callable` que lanza una excepción
    genérica). Paridad: 612 de 633 y 212 de 219, sin avisos de más. Además de los archivos sin
    parsear y las carpetas ocultas, faltan los módulos de la biblioteca estándar que se usan a sí
    mismos (`open` dentro de `gzip.py`): ruff conoce el nombre del módulo de cada archivo y resuelve
    esas definiciones como `gzip.open`; kerbo no lo calcula. Los casos quedaron en
    `fixtures/resources.py`.
  - Hecha UP035 (imports de módulos de los que el nombre se mudó): las tablas de
    `deprecated/tables.rs` salen del fuente de ruff para Python 3.11, que es la versión por defecto
    de ruff 0.17 (lo confirma que `typing_extensions.override`, de 3.12, no se reporta). Un aviso
    por destino y uno por cada miembro de `typing` que cambió de nombre, como ruff. Paridad: 1.168 de
    1.169, sin avisos de más; el que falta es un `lazy from` de CPython.
  - Hechas ISC004 (concatenación implícita sin paréntesis dentro de una lista, tupla, conjunto o
    subíndice con varios elementos) y SIM114 (ramas seguidas de un `if` con el mismo cuerpo). SIM114
    aproxima el `ComparableStmt` de ruff comparando el árbol sin comentarios ni paréntesis de
    agrupación y los strings por su valor, y compara los comentarios en el mismo rango que
    `body_range` (desde la línea siguiente a la condición). Paridad: 947 de 947 y 245 de 245.
  - Hecha B023 (función o lambda en un loop que usa una variable del loop), sintáctica como en
    ruff: los nombres que lee cada función o lambda sin ser parámetros ni asignarlos, contra los que
    el loop asigna (destinos de asignaciones, `for` y comprensiones, sin entrar a funciones ni
    lambdas), con las lambdas que se consumen enseguida como seguras. `loops/names.rs` decide si un
    identificador de tree-sitter es una lectura, una escritura o no es un nombre en el AST de ruff.
    Paridad: 583 de 583.
  - Hechas RUF022 (`__all__` sin ordenar, en el módulo: asignación, `+=` y `.extend`) y RUF023
    (`__slots__` sin ordenar, en una clase: lista, tupla, conjunto o claves de un diccionario), con un
    port de `natord` y del orden "isort" de ruff (constantes, clases y el resto, cada grupo en orden
    natural). Paridad: 406 de 406 y 311 de 311. De paso, F822 lee también los nombres de `__all__`
    en una tupla sin paréntesis y en strings concatenados.
  - Hechas en una tanda de reglas chicas: FURB167 (alias de una letra de los flags de `re`),
    PLR2044 (comentario vacío, con los bloques de comentarios de ruff), SIM201 (`not a == b`),
    PLW1510 (`subprocess.run` sin `check`), PLR0124 (comparar algo consigo mismo), PLR0133 (comparar
    dos constantes), C401 y C402 (`set`/`dict` sobre un generador) y LOG015 (logger raíz). Paridad
    exacta salvo un C401 de CPython con `await` adentro del generador y dos casos en archivos sin
    parsear.
  - Hechas FLY002 (`" ".join([...])` fijo, con las reglas de flynt sobre qué elementos entran en
    una f-string y las comillas), PIE804 (`f(**{"a": 1})`) y RUF010 (`str()`, `repr()` o `ascii()`
    adentro de una f-string). Paridad: 242 de 242, 102 de 102 y 105 de 105.
  - Hecha I001 (bloque de imports sin ordenar o formatear), con un port del isort de ruff: arma el
    bloque esperado (secciones de futuro, estándar, terceros, del proyecto y relativos; imports del
    mismo módulo fusionados; comentarios arriba, en línea y al final de cada nombre; corte a 88
    columnas con la coma mágica) y lo compara con el texto, sin mirar la sangría. Respeta
    `# isort: skip_file`, `off`/`on`, `skip` y `split`, las líneas que siguen al bloque (dos antes
    de una `def` o `class`, una en los stubs) y los bloques que comparten línea con otra sentencia.
    La lista de la biblioteca estándar sale de ruff para 3.11 (`imports/stdlib.rs`) y lo propio se
    decide como ruff con la configuración por defecto: el paquete del archivo o un módulo que
    existe bajo `.` o `src`. Paridad contra `--isolated`: 6.917 de 7.021, sin avisos de más. Lo que
    falta: 64 archivos de CPython con `lazy import`, los que tree-sitter no parsea, la carpeta
    oculta `.ci` de sympy y un `.pyi` de pydantic-core sin explicar todavía. Los casos quedaron en
    `fixtures/isort.py` y `fixtures/isort_unsorted.py`.
  - Hecha UP032 (`"{}".format(x)` que puede ser f-string), con un port del parser de formatos de
    `ruff_python_literal` sobre el texto de cada parte del string (con `\N{…}` y el prefijo `r`):
    argumentos sin `*`/`**`, comillas ni saltos de línea, campos que apuntan a argumentos que
    existen, sin repetir uno con efectos, y nada adentro de un `gettext`/`gettext_lazy` de Django.
    De paso, el resumen de F521–F525 saltea `\N{…}` como ruff (daba un F524 de más en un string
    crudo). Paridad: 1.851 de 1.920, sin avisos de más; lo que falta está en archivos de CPython con
    `lazy import`, en `test_patma.py` y en una carpeta oculta de ansible. Los casos quedaron en
    `fixtures/format_calls.py`. I001 sale del preset `minimal`, con las demás reglas de estilo.
  - Hechas N999 (nombre de módulo, sólo adentro de un paquete; un `__init__.py` toma el nombre del
    paquete raíz y en `migrations`/`versions` se permite empezar con un dígito), SIM103 (`if` que
    devuelve `True`/`False`, con `else`, con el último `elif` y su `else`, o con el `return` que
    sigue), SIM905 (`split`/`rsplit` sobre un string fijo) y FURB157 (`Decimal("10")` y
    `Decimal(float("nan"))`). Paridad: 155 de 156, 162 de 162, 162 de 162 y 161 de 161, sin avisos
    de más; el N999 que falta es un archivo oculto (`.util.py`). Los casos quedaron en
    `fixtures/statics.py` y N999 en un test con un paquete en disco.
  - RUF100 (`noqa` sin usar, 1.437) queda para cuando kerbo tenga el conjunto por defecto completo:
    decidir si un código de un `noqa` se usa pide correr esa regla, y con un conjunto parcial
    marcaría como sobrante un `# noqa: B904` que en ruff sí silencia algo.
  - RUF046 (`int()` sobre algo que ya es entero) necesita un port de `ResolvedPythonType`, la
    inferencia de tipos de expresiones de ruff, y de los tipos de los parámetros de `round`.
  - Hechas UP034 (paréntesis de más, con el recorrido por tokens de ruff sobre las hojas del árbol),
    PLC0205 (`__slots__` con un único string), PLW0120 (`else` de un loop sin `break`) y RET501
    (`return None` en una función que sólo devuelve `None`, sin generadores, propiedades ni retornos
    anotados con otro tipo). Paridad: 145 de 145, 86 de 86, 83 de 83 y 82 de 86, sin avisos de más;
    los RET501 que faltan están en archivos que tree-sitter no parsea. Los casos quedaron en
    `fixtures/pylint_rules.py`.
  - Hechas PLR1711 (`return` inútil al final), PLR0402 (`import a.b as b`), B026 (`*args` después
    de un argumento con nombre), UP039 (`class C():`), UP020 (`io.open`) y RUF032 (`Decimal(0.1)`).
    Paridad: 65 de 66, 56 de 56, 56 de 56, 40 de 40, 53 de 55 y 55 de 55, sin avisos de más; lo que
    falta está en archivos de CPython con `lazy import`. Los casos quedaron en
    `fixtures/declarations.py`.
  - Hechas TRY201 (`raise error` en su propio `except`), UP012 (`"x".encode("utf-8")`), C400
    (`list(x for x in y)`), C409 (`tuple([1, 2])`), C413 (`list(sorted(x))`) y C419
    (`any([x for x in y])`). Paridad: 45 de 45, 61 de 61, 33 de 34, 24 de 24, 21 de 22 y 24 de 24,
    sin avisos de más. Faltan un C413 en un archivo con `lazy import` y un C400 en
    `list(await 2 for _ in [])` de `test_builtin.py`, que tree-sitter no arma como generador. Los
    casos quedaron en `fixtures/collections_calls.py`. Con 128 reglas, `wrap_python` (que arma la
    lista de reglas en tiempo de compilación) pasó a ser un loop: la recursión superaba el límite
    de llamadas anidadas de la evaluación `const`.
  - Hechas T100 (depuradores, en llamadas e imports), B017 (`assertRaises(Exception)` y
    `pytest.raises(Exception)`), B002 (`++x`), PLW0127 (`x = x`) y PLW0133 (una excepción construida
    sin `raise`). Paridad: 39 de 40, 47 de 47, 25 de 25, 34 de 34 y 24 de 24, sin avisos de más. El
    T100 que falta es `from . import embed` adentro de `IPython/terminal/`: ruff resuelve el import
    relativo con el nombre de módulo del archivo, que kerbo todavía no calcula (como en SIM115). De
    paso, F823 deja de mirar los alcances de clase al buscar qué nombre de afuera pisa una variable
    local (no se ven desde adentro de sus métodos). Los casos quedaron en `fixtures/suspicious.py`.
  - Hechas TRY203 (`try` cuyos `except` sólo vuelven a lanzar), PLE0704 (`raise` a secas fuera de
    un `except`; una función definida adentro de uno ya no cuenta como adentro), SIM210 (`True if c
    else False`) y B020 (la variable del `for` pisa lo que se recorre, con el `NameFinder` de ruff).
    Paridad: 37 de 37, 33 de 33, 27 de 27 y 44 de 44. Los casos quedaron en
    `fixtures/exception_flow.py`; los textos, en un catálogo nuevo (`i18n/*/python/checks.rs`).
  - Hechas UP024 (alias viejos de `OSError`) y UP041 (de `TimeoutError`, para 3.11) en el `except`,
    el `raise` y la llamada; UP003 (`type("")`), UP022 (`stdout` y `stderr` en `PIPE`), B004
    (`hasattr(x, "__call__")`) y UP011 (`@lru_cache()`). Paridad: 34 de 35, 16 de 18, 17 de 17, 17
    de 17, 23 de 23 y 29 de 30, sin avisos de más; lo que falta está en archivos de CPython con
    `lazy import`. Los casos quedaron en `fixtures/aliases.py`.
  - Hechas SIM202 (`not a != b`), PIE794 (atributo de clase repetido), PIE807 (`lambda: []`),
    PLW0128 (el mismo nombre dos veces entre los destinos) y PLW0642 (asignar a `self` o `cls`, con
    el `function_type::classify` de ruff y los decoradores reconocidos por nombre). Paridad exacta:
    18, 20, 12, 19 y 68. Los casos quedaron en `fixtures/class_bodies.py`; la tabla de fixtures pasó
    a `src/lint/python/tests/fixtures.rs`.
  - Hechas PLW0211 (método estático cuyo primer parámetro es `self` o `cls`), PLC3002 (`(lambda:
    x)()`) y DTZ002 (`datetime.today()` sin `.astimezone()` después). Paridad exacta: 20, 15 y 18.
    Los casos quedaron en `fixtures/methods.py`.
  - Hechas S112 (`try`/`except`/`continue`), C410 (`list([...])`), C406 (`dict([...])`), DTZ003
    (`datetime.utcnow()`), PLR1722 (`exit()` y `quit()`), UP021 (`universal_newlines` de
    `subprocess`), PLW1509 (`preexec_fn`), UP033 (`lru_cache(maxsize=None)`), B003 (asignar a
    `os.environ`) y PLC0208 (recorrer un conjunto escrito). Paridad: 7 de 8 S112 y exacta en el
    resto (10, 9, 10, 9, 7, 7, 8, 10 y 8); el S112 que falta está en un archivo de CPython con
    `lazy import`. Los casos quedaron en `fixtures/builtins_misc.py`.
  - Hechas UP037 (comillas de más en una anotación que Python no evalúa: con el import del futuro,
    en un stub o en una variable local de una función; también en un string adentro de otro) y
    UP044 (`Unpack[X]` adentro de un subíndice o como tipo de `*args`, también entre comillas).
    Paridad exacta: 172 y 183. Los casos quedaron en `fixtures/quoted.py`,
    `fixtures/quoted_future.py` y `fixtures/unpack.py`.
  - Pendientes que piden más análisis: PIE810 (si un nombre está ligado a una tupla, con el
    `check_type` de ruff), UP028 (las referencias de cada nombre al cerrar el alcance) y C414.
  - Lo que queda del conjunto por defecto (2026-10-24): 141 reglas con unos 3.300 avisos en el
    corpus, contra más de 100.000 ya cubiertos. RUF100 (`noqa` sin uso) suma 1.437; el resto tiene
    menos de 100 cada una: RUF046 107, RUF013 96, PLR1704 87, PLC0206 78, PYI061 70, PYI032 63,
    PLR1730 56, PYI063 54, UP028 49, PYI036 47, PIE810 46 y una cola de reglas con menos de 45.
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
  - I001: el aviso que falta en `pydantic-core/python/pydantic_core/_pydantic_core/__init__.pyi`
    (ruff lo da y kerbo no; sospecha: cómo se clasifica `pydantic_core` desde un paquete que sólo
    tiene `__init__.pyi`).
  - Arreglos que ruff tiene y kerbo todavía no: I001 (reescribir el bloque ordenado), E401 (partir
    el import), F811 (borrar la definición pisada), F504/F522/F523 (sacar los argumentos de más) y
    E731 (pasar la lambda a `def`).
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
    `no-dupe-else-if` y `no-dupe-keys` (las siguientes, más abajo).
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
- `no-redeclare` y `no-func-assign` (hecho, 2026-10-22), sobre el motor de alcances que ya existía:
  la primera avisa en cada declaración de una variable después de la primera, en los alcances que
  mira ESLint (programa, funciones, bloques, `for`, `switch` y bloques estáticos; no los `catch` ni
  el nombre de una función con nombre); la segunda, en cada escritura que no es la inicialización a
  una variable cuya primera declaración es una función. Como en typescript-eslint, las dos se apagan
  en TypeScript. Paridad contra ESLint en express, zod, react, vscode y next.js: 149 de 149 y 132
  de 132, sin avisos de más ni de menos. El ejemplo `fixtures/redeclare.js` fija los casos (función
  y `var`, parámetro y `var`, bucles, `catch`, asignación compuesta y desestructuración).
- `no-wrapper-object-types`, `no-unsafe-function-type`, `triple-slash-reference`,
  `no-unsafe-finally` y `prefer-spread` (hecho, 2026-10-23), las cinco que no piden el grafo de
  control:
  - Las dos de tipos deciden si el nombre es el global como `isReferenceToGlobalFunction` de
    typescript-eslint: miran sólo la primera referencia con ese nombre creada en el alcance del uso
    (un `type Boolean` declarado más abajo en el módulo exime a todos los `Boolean` del módulo, y un
    `const Number` no exime a un tipo). `no-wrapper-object-types` trae el arreglo de ESLint (el
    nombre en minúscula), salvo en `implements` y `extends` de una interfaz, igual que ESLint.
  - `triple-slash-reference` con las opciones de fábrica: `path` siempre avisa, `types` sólo si el
    mismo módulo se importa (un aviso por cada import, con `import x = require()` incluido), `lib`
    nunca; sólo los comentarios antes del primer token.
  - `no-unsafe-finally` y `prefer-spread`, calcadas de ESLint: el corte en funciones, clases, loops
    y `switch` según la sentencia y su etiqueta; el `this` esperado comparado token a token.
  - Paridad contra ESLint en express, zod, react, vscode y next.js: 152 de 152, 107 de 113, 70 de
    70, 36 de 36 y 40 de 40, ninguno de más. Los 6 que faltan están en archivos que kerbo no
    analiza (el `webpack-config.ts` de next.js, que tree-sitter no parsea, y tres `.js` de
    react-devtools con anotaciones). La frecuencia de abajo contaba también el código compilado
    que next.js ignora; por eso daba más. Los casos quedaron en `fixtures/wrappers.ts`,
    `finally.js`, `spread.js` y `applies.ts`.
- `no-constant-condition` y `no-constant-binary-expression` (hecho, 2026-10-23), sobre un port de
  `isConstant` y `isLogicalIdentity` de ESLint (`src/lint/ecmascript/constants.rs`) que mira el
  árbol como ESTree: sin paréntesis, `&&`, `||` y `??` como expresiones lógicas, `as`, `!` y
  `satisfies` como lo que no es constante, y las llamadas opcionales (`Boolean?.(x)`) como cadenas.
  `undefined`, `Boolean`, `String` y compañía cuentan como globales si ninguna declaración los
  tapa. `no-constant-condition` va con `checkLoops: "allExceptWhileTrue"` y perdona un loop con un
  `yield` del mismo generador adentro (en un `for`, el de la inicialización no cuenta, como en
  ESLint). Paridad en express, zod, react, vscode y next.js: 155 de 155 y 27 de 27, sin avisos de
  más. Los casos quedaron en `fixtures/conditions.js` y `fixtures/binaries.js`.
- `no-const-assign`, `no-class-assign`, `no-import-assign`, `no-global-assign`,
  `no-shadow-restricted-names` y `no-unassigned-vars` (hecho, 2026-10-23), sobre el motor de
  alcances: las escrituras que no inicializan (una por identificador, como
  `getModifyingReferences`), los miembros de un `import * as` que se pisan, se borran, se iteran o
  pasan por `Object.assign`/`Reflect.set` y compañía, los globales de ECMAScript de sólo lectura
  (la misma lista que ESLint) y los `let`/`var` sin valor que se leen y nunca se escriben (no los
  de `declare`). Las tres primeras se apagan en TypeScript, como en typescript-eslint. Paridad en
  express, zod, react, vscode y next.js: 11 de 11, 2 de 2, 3 de 3, 1 de 1, 34 de 36 y 11 de 23, sin
  avisos de más. Faltan los de dos archivos que tree-sitter no parsea (`base-server.ts` y
  `router.ts` de next.js) y 12 de una salida de tests de next.js que redeclara cuatro veces el mismo
  import como `const` (código inválido). Los casos quedaron en `fixtures/assignments.js` y
  `fixtures/declared.ts`.
- `valid-typeof`, `no-empty-pattern`, `no-sparse-arrays`, `require-yield`, `no-useless-catch`,
  `no-compare-neg-zero`, `no-async-promise-executor`, `use-isnan`, `no-unused-labels` y
  `no-self-assign` (hecho, 2026-10-23), con las opciones de fábrica (`use-isnan` mira los `switch`
  y no `indexOf`; `no-self-assign` mira también las propiedades, con el `isSameReference` y el
  `getStaticPropertyName` de ESLint en `src/lint/ecmascript/self_assign.rs` y `members.rs`). Paridad
  en express, zod, react, vscode y next.js: 4 de 4, 21 de 21, 8 de 8, 14 de 15, 10 de 10, 5 de 5,
  8 de 8, 4 de 4, 4 de 4 y 3 de 3, sin avisos de más; el que falta está en un test de Copilot que
  tree-sitter no parsea. Los casos quedaron en `fixtures/checks.js`.
- `no-setter-return`, `no-unsafe-optional-chaining`, `no-non-null-asserted-optional-chain`,
  `preserve-caught-error`, `no-extra-boolean-cast`, `no-namespace` y `no-array-constructor`
  (hecho, 2026-10-23), con las opciones de fábrica. Un `ChainExpression` de ESTree se reconoce en
  tree-sitter como un acceso o una llamada con `?.` en su cadena que no sigue encadenado sin
  paréntesis (`src/lint/ecmascript/optional_chains.rs`). `no-setter-return` mira también los
  descriptores de `Object.defineProperty` y compañía, y se apaga en TypeScript. `no-array-constructor`
  trae el arreglo de typescript-eslint (el literal con los mismos argumentos); `no-extra-boolean-cast`
  todavía no, porque el de ESLint decide los paréntesis por precedencia. Paridad en express, zod,
  react, vscode y next.js: 23 de 23, 24 de 25, 1 de 1, 3 de 3, 4 de 4 y 64 de 66, sin avisos de
  más (no-array-constructor no aparece); los que faltan están en archivos de next.js que tree-sitter
  no parsea. Los casos quedaron en `fixtures/errors.js` y `fixtures/namespaces.ts`.
- El análisis de caminos de ESLint (`code-path-analysis`) portado sobre tree-sitter (hecho,
  2026-10-23), en `src/lint/ecmascript/paths/`: segmentos y contextos de bifurcación con sus
  caminos dobles adentro de un `finally`, los contextos de elección (`&&`, `||`, `??`, `if`,
  ternarios, cadenas opcionales), loops con condiciones literales, `switch` con el `default` en el
  medio, `try`/`catch`/`finally` con el primer punto que puede tirar, etiquetas, y un camino por
  programa, función, bloque estático e inicializador de campo. El recorrido traduce el árbol de
  tree-sitter a los nodos de ESTree (sin paréntesis, un método como clave más función, la cadena
  opcional como `ChainExpression`) y lleva la pila de ancestros, porque `Node::parent` de tree-sitter
  busca desde la raíz y volvía cuadrático el análisis en archivos grandes (next.js tardaba 152 s en
  vez de 60). Encima, `no-unreachable` (con los rangos de sentencias seguidas y los campos de una
  clase derivada sin `super()`) y `no-fallthrough` (con el comentario `falls through` y las líneas en
  blanco de un `case` vacío). Paridad en express, zod, react, vscode y next.js: 37 de 37 y 51 de 51,
  sin avisos de más. Los casos quedaron en `fixtures/reachability.js`.
- `no-useless-assignment` (hecho, 2026-10-23), sobre el análisis de caminos: cada camino guarda los
  segmentos alcanzables activos en cada asignación, el primer y el último identificador de cada
  segmento y sus bloques `try`, y la regla recorre los segmentos siguientes como ESLint (sin pasar
  de uno que vuelve a asignar, con las lecturas desde otra función como uso y las variables
  exportadas afuera). Paridad en express, zod, react, vscode y next.js:
  599 de 602, sin avisos de más; los tres que faltan están en archivos que tree-sitter no parsea. La
  frecuencia de arriba contaba el código compilado que next.js ignora. Los casos quedaron en
  `fixtures/assigned.js`.
- `getter-return` (hecho, 2026-10-23): getters de clases y objetos y los de los descriptores de
  `Object.defineProperty` y compañía (como `no-setter-return`), con el `return;` vacío y la salida
  alcanzable del análisis de caminos. Se apaga en TypeScript. Paridad: 11 de 11. Los casos quedaron
  en `fixtures/getters.js`.
- `no-loss-of-precision` y `no-control-regex` (hecho, 2026-10-23): el número escrito contra el que
  guarda el `double` (en base 10 con la precisión pedida, en las otras bases por los bits
  significativos), y los caracteres de control de un patrón (literal o primer argumento de
  `RegExp`) escritos tal cual o con `\x` y `\u`. Paridad: 14 de 14 y 33 de 33. Los casos quedaron
  en `fixtures/literals.js`.
- `no-unexpected-multiline` y `no-unused-private-class-members` (hecho, 2026-10-23): la línea que
  sigue a la anterior por un `(`, `[`, template o `/…/flags` sin punto y coma (una flecha sin
  paréntesis como lo llamado es un error de tree-sitter, que no ve el punto y coma automático), y
  los `#miembros` que la clase sólo escribe, incrementa como sentencia o desestructura. Paridad: 2
  de 2 y 3 de 3. Los casos quedaron en `fixtures/multiline.js`.
- Pendientes de TypeScript y JavaScript contra ESLint, por frecuencia:
  - Sobre el análisis de caminos que ya está: `no-this-before-super` (9) y `constructor-super` (7).
  - `no-misleading-character-class` (290, casi todo en código compilado),
    `no-irregular-whitespace` (10) y las que no aparecieron en el corpus.
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
- staticcheck ST1005 (hecho, 2026-10-22): `st1005`, mensajes de `errors.New` y `fmt.Errorf` (por la
  ruta del import, con alias) que terminan en `.`, `:`, `!` o salto de línea, o que empiezan con
  mayúscula. El mensaje tiene que ser constante: literales, sumas de literales y constantes del mismo
  archivo, con los escapes de Go. Como staticcheck, no avisa con una sola palabra, con una sigla o un
  nombre compuesto (`HTTP`, `ReadAll`), con un `(`, ni si la primera palabra es el nombre de una
  función, tipo, variable o método del paquete (también `Tipo.Metodo`); las constantes no eximen. En
  los `_test.go` sólo mira lo de fuera de las funciones (la inicialización de las variables del
  paquete). `//nolint:staticcheck` la silencia.
  - Lo que pide el paquete entero se resuelve así: cada archivo marca todos sus mensajes con mayúscula
    (y pasan por `//nolint` como cualquier aviso) y guarda en la caché su paquete, lo que declara y
    dónde están; al final del escaneo se descartan los que nombran algo del paquete (carpeta y
    nombre de `package`) y los de dentro de funciones de test. Los archivos generados no avisan pero
    sus nombres cuentan, como en golangci-lint.
  - Paridad contra golangci-lint (la corrida guardada del conjunto por defecto, en los archivos de
    linux/amd64): 1.073 coincidencias por archivo y línea, ninguno de más y 10 de menos, todos de
    terraform con el mensaje en una constante de otro archivo del paquete (`EnvInvalidName`,
    `errInitConfigError`). consul 840 de 840, hugo, golang/tools y fzf completos; caddy, chi, cobra,
    etcd, gin y prometheus sin avisos en los dos.
- Pendientes de Go contra golangci-lint:
  - ST1005 con el mensaje en una constante de otro archivo del paquete: pide guardar las constantes
    de texto de cada archivo y evaluar el mensaje en la pasada por paquete. Tampoco se verificó si las
    variables del paquete eximen (en el corpus no hubo caso; se eximen, como los tipos).
  - Las de staticcheck que tampoco piden tipos: QF1004, QF1001 y QF1003.
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

### 12. Migraciones y contenedores (cerrada)

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
- Lo que quedaba para cerrarla, todo hecho o movido de etapa:
  - Los renombres que el código desplegado todavía usa pasaron a la etapa 9 (piden el índice del
    proyecto y el esquema del ORM), y las migraciones de Django y Rails, a la etapa 14.
  - El resto de squawk (hecho, 2026-10-21): las 40 reglas que squawk 2.70 trae prendidas y kerbo
    no tenía, con su criterio portado del código de squawk (licencia Apache-2.0/MIT; la lista de
    funciones no volátiles de `adding-field-with-default` está copiada en
    `src/deploy/sql/squawk/non_volatile_functions.txt` con su origen). Queda afuera
    `prefer-repack`, que squawk sólo corre con Postgres 19 y con el 15 de fábrica nunca marca.
    - Nuevas: los timeouts (`require-lock-timeout`, `require-statement-timeout`, sobre la primera
      sentencia que squawk considera lenta), las restricciones (`constraint-missing-not-valid`,
      `adding-foreign-key-constraint`, `adding-serial-primary-key-field`,
      `disallowed-unique-constraint`), `adding-field-with-default` con el mismo criterio de
      expresiones no volátiles (`-1` y `(0)` cuentan como volátiles, `interval '1 day'` no), los
      tipos (`prefer-bigint-over-int`, `prefer-bigint-over-smallint`, `prefer-identity`,
      `prefer-text-field`, `prefer-timestamp-tz`, `ban-char-field`), los borrados
      (`ban-drop-constraint`, `-default`, `-not-null`, `-generated-expression`, `-type`,
      `-function`, `-sequence`, `-view`, `-schema`, `-domain`, `-extension`, `-database`),
      `renaming-object`, `ban-set-schema`, `require-enum-value-ordering`, `ban-alter-identity`, los
      dominios, `require-concurrent-reindex`, `require-concurrent-partition-detach`,
      `ban-truncate-cascade`, las de transacciones (`transaction-nesting`,
      `ban-uncommitted-transaction`, `ban-concurrent-index-creation-in-transaction`,
      `prefer-robust-stmts`, que reconoce las transacciones de Alembic con `alembic_version`),
      `identifier-too-long` y `ban-duplicate-column-assignments`, con la inferencia de nombres de
      columna de squawk para vistas y `SELECT INTO` (`count(*)` se llama `count`, `t.a` se llama
      `a`, un literal `?column?`).
    - Frecuencia en el corpus: los timeouts en casi cada archivo (1.730), `ban-drop-constraint`
      896, `prefer-timestamp-tz` 882, `prefer-bigint-over-int` 830,
      `constraint-missing-not-valid` 480, `adding-foreign-key-constraint` 412, `prefer-text-field`
      277 y el resto con menos de 170; 15 de las reglas no aparecen ni una vez.
    - Paridad contra squawk 2.70.0 (`--assume-in-transaction`, 2026-10-21) en las 1.858
      migraciones de lobe-chat, documenso, trigger.dev, cal.com, conductor y awesome-compose:
      **9.612 de 9.612 avisos, ninguno de más**, con las nueve reglas anteriores. Las que no
      aparecen en el corpus están fijadas con la salida de squawk en `src/deploy/sql/fixtures/`.
    - `recommended` apaga las de preferencia de tipos (`prefer-*` y `ban-char-field`) y
      `minimal`, también los timeouts.
  - Alembic con todas las reglas (hecho, 2026-10-21): cada `op.*` y `batch_op.*` se convierte en
    el SQL que Alembic generaría para Postgres (calibrado contra Alembic 1.13 en modo offline:
    tipos de SQLAlchemy, `server_default`, `SERIAL` para la clave primaria entera única, el
    `CREATE TYPE` de los `Enum` de `create_table`, las claves foráneas y los índices de
    `add_column` como sentencias aparte) dentro del mismo marco de transacción que arma Alembic
    (`BEGIN`, `COMMIT`/`BEGIN` alrededor de `autocommit_block()` y la actualización de
    `alembic_version`), y el script pasa por las mismas 49 reglas que un `.sql`; lo que cae en el
    marco se descarta y el resto vuelve a la línea de la llamada. Se expanden los `for` sobre listas
    literales o constantes del módulo, se siguen las funciones auxiliares del mismo archivo, las
    constantes y variables locales (tipos y columnas) y los f-strings con constantes; se saltean
    los `except`, las ramas de otro dialecto (también `not isinstance(..., SQLiteDialect)`) y lo que
    sigue a un `return` de la rama de Postgres.
    - Paridad contra squawk 2.70 sobre el SQL que genera Alembic en modo offline, por revisión, regla
      y cantidad, descontando lo que cae en el marco: 300 revisiones de airflow, superset, mlflow y
      redash, 297 idénticas y ninguna con avisos de menos. Las dos con avisos de más toman caminos
      que el modo offline no recorre (`is_offline_mode()`, y un `drop_constraint(None)` que en
      offline corta el `try`); la restante tiene SQL que squawk no parsea. Las otras 349 revisiones
      importan la aplicación o consultan la base y no se convierten sin levantarla: el límite es de
      la medición, la conversión de kerbo no depende de eso.
  - Las reglas de hadolint que faltaban (hecho, 2026-10-21): DL3001, DL3010, DL3011, DL3028,
    DL3030, DL3033 a DL3038, DL3040, DL3041, DL3043, DL3044, DL3046, DL3048 (que no depende de la
    configuración, aunque el README la agrupaba con DL3049 a DL3058), DL3060, DL3063, DL3065 y
    DL4005, portadas del código de hadolint con sus rarezas: DL3010 sólo reporta los archivos
    copiados y descomprimidos en la última etapa (un `FROM` descarta lo anterior), DL3060 sólo
    en la última etapa o en una que otra usa de base, y DL3033/DL3041 heredan las variables `ENV`
    de la etapa base. Quedan afuera las que hadolint trae apagadas o sólo corren con su
    configuración (DL1001, DL3026, DL3049 a DL3058); `.hadolint.yaml` no se lee, a propósito: las
    excepciones van en `kerbo.toml` con motivo. Paridad contra hadolint 2.15.1 (configuración por
    defecto, pragmas respetados): 408 de 408 avisos en 169 Dockerfiles de los mismos diez repos;
    lo que el corpus no ejercita está fijado con la salida de hadolint en
    `src/deploy/dockerfile/fixtures/`. ShellCheck sobre el shell de cada `RUN` es de la etapa 10.
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
  - Lo que faltaba de las supresiones en el código (hecho, 2026-10-21):
    - `squawk-ignore` vale como en squawk: para la línea del comentario y la siguiente, también al
      final de una línea o en un `/* */`, y lo que sigue a otro `--` es el motivo. Sin reglas no
      silencia nada; `squawk-ignore-file` sin reglas silencia todas las de migraciones. Los
      comentarios salen del lexer de SQL, así que un `--` dentro de un string no cuenta. (La nota
      vieja decía que squawk asocia el comentario a la sentencia siguiente: no es así.)
    - `unused-ignore`, la regla de squawk que marca un nombre de regla que no existe en un
      `squawk-ignore`, en la posición del nombre.
    - El SARIF manda lo silenciado como resultados con `suppressions` (`kind: inSource` y el
      motivo como `justification`), así GitHub los muestra como descartados.
  - compose (hecho, 2026-10-21): `compose-cpu-limit`, `compose-restart`, `compose-image-tag`
    (sin tag o con `latest`, como DL3006/DL3007; un digest o una variable no cuentan) y
    `compose-depends-on-healthy` (un `depends_on` hacia un servicio con healthcheck activo que no
    espera `service_healthy`). Criterio propio, como las tres primeras; `minimal` las apaga.

### 13. Formateo propio (empezada: Go medido contra gofmt)

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
- Go (2026-10-22; la primera versión la armó un agente en una rama aparte que quedó cortada):
  `kerbo fmt [rutas]` reescribe los `.go` y `kerbo fmt --check` sólo avisa cuáles cambiarían. No
  reimplementa gofmt a ojo: arma desde tree-sitter el mismo AST que `go/parser` y lo imprime con un
  port de `go/printer` (`src/formatter/go/`: tabwriter, comentarios, cortes de línea, orden de
  imports, paréntesis que se sacan de las condiciones). `new(expresión)` de Go 1.26, que
  tree-sitter-go todavía no conoce, se parsea cambiando `new` por un nombre del mismo largo.
  - Un archivo que tree-sitter-go no lee, o que lee distinto que Go, queda como está y se avisa:
    formatearlo cambiaría el programa. Las lecturas distintas que se detectan: `if x {...}` seguido
    de un bloque suelto (tree-sitter toma `x{...}` como literal), `*(*T)(p).f` (lo lee como el campo
    de una conversión al tipo `*(*T)`) y `chan<- chan T` (lo lee como un `chan` de `<-chan T`).
    Las dos últimas, sin la guarda, cambiaban el significado del archivo.
  - Paridad contra gofmt (2026-10-22) sobre 19.143 archivos (la biblioteca estándar de Go, golang/
    tools, terraform, consul, hugo, caddy, etcd, prometheus, syncthing, tailscale, gin, cobra, chi y
    fzf), en tres versiones: el original, uno desordenado a propósito (sangrías, espacios y líneas en
    blanco cambiados sin tocar textos ni comentarios) y el desordenado con CRLF. Idénticos a gofmt:
    19.043 de 19.071 originales, 19.018 de 19.068 desordenados y 19.030 de 19.068 con CRLF (los que
    gofmt mismo no parsea, excluidos). Formatear dos veces no cambia nada salvo en 7 archivos.
  - Lo que falta, por tipo, en los originales: 16 archivos que tree-sitter-go no lee (sintaxis nueva
    como los métodos genéricos, o las lecturas distintas de arriba) y 12 comentarios de
    documentación que gofmt reescribe desde Go 1.19 (`go/doc/comment`: `//texto` pasa a `// texto`,
    las sangrías a bloques de código con tab y las listas se reacomodan) y kerbo deja como están. En
    los desordenados se suman los mismos comentarios.
  - Lo que se arregló con esta medición: un grupo de comentarios con `/* */` en posición de
    documentación ya no se junta en una línea, las directivas (`//go:noinline`, `//export`) van al
    final de la documentación separadas por `//` y el texto pierde las líneas en blanco de los
    bordes, y las líneas en blanco entre el último import y el `)` se juntan como en `SortImports`.
  - El arnés (desordenar, gofmt y kerbo sobre el mismo corpus y comparar archivo por archivo) corre
    entero en un volumen de Docker, así no choca con el largo máximo de las rutas de Windows; quedó
    fuera del repo, como los demás.
  - Pendientes: el reformateo de `go/doc/comment`, y recién después de eso recomendar `kerbo fmt`.
    Después de Go: ruff format (Python), Prettier (JS/TS) y google-java-format (Java).
- Con el formateador, compilar los tests con cobertura queda al borde de los 1,5 GB del gate: sin
  la caché caliente lo mata el límite de memoria (SIGKILL). Pasa, pero hay que ganar margen
  (partir el crate o bajar la información de depuración de los tests) antes de que crezca más.
- El test intermitente de osv (`reports_what_curl_says_when_it_fails`, "Broken pipe" cuando el curl
  falso termina antes de leer el cuerpo) quedó arreglado (2026-10-22): si curl sale antes de leer, la
  escritura se ignora y vale lo que curl dice al salir. Pasaba igual con el curl de verdad.

### 14. Más migraciones: Django y Rails

- Migraciones de Django (sobre el árbol de Python que ya existe: `AddIndex` sin
  `AddIndexConcurrently`, `AddField` con `null=False` sin default, `RemoveField`, `RenameField`) y
  de Rails (`add_index` sin `algorithm: :concurrently`, `remove_column`, `change_column`), con
  strong_migrations como referencia de Rails. Rails pide sumar una gramática de Ruby. Viene de la
  etapa 12.

### 15. Java: Javadoc, paridad con PMD y Qodana, y la API nueva

Lo que quedó de la etapa 8 cuando se cerraron las reglas de forma de `sun_checks.xml`.

- Las reglas de Javadoc de `sun_checks.xml`, por frecuencia sobre Guava, spring-boot,
  commons-lang y junit4: `JavadocVariable` (29.407), `MissingJavadocMethod` (27.721),
  `JavadocMethod` (19.370), `JavadocType` (1.921), `JavadocPackage` (1.508),
  `InvalidJavadocPosition` (13). Piden leer el Javadoc como árbol; tree-sitter lo deja como un
  comentario.
- La misma paridad contra PMD para quien usa el quickstart en vez de Checkstyle:
  `UnnecessaryImport` resuelve tipos con el classpath y no va a coincidir sin las firmas de la
  etapa 9; `UnnecessaryModifier` y `ClassWithOnlyPrivateConstructorsShouldBeFinal`, sí.
- Sumar Qodana al arnés de paridad para las reglas que coinciden con una inspección (imports sin
  usar y modificadores redundantes).
- Por lo que más aparece en Qodana: la API nueva que reemplaza a la vieja (`getFirst()`,
  `List.of`, `Math.clamp`, bloques de texto) y las redundancias (valor por defecto repetido en una
  anotación, escapes Unicode de más).

### 16. Tipos y firmas completas

Lo que la etapa 9 dejó porque pide saber más que lo que se ve en el archivo y el índice.

- Las excepciones de las firmas (`Exceptions` de cada método de los `.class`) y, con eso,
  `RedundantThrows` de IntelliJ (28 en Qodana, casi todos `throws Exception` en tests).
- El JDK desde `ct.sym` para el `release` del proyecto, en vez de la tabla propia.
- Tipos genéricos: `Map.get` y lo que devuelve una lambda en el flujo de nulos.
- El tipo de retorno de los métodos del proyecto, para seguir cadenas que pasan por ellos, y la
  inferencia de parámetros no nulos para todos los métodos (hoy sólo los `@Override`), para avisar
  de un `null` pasado a uno.
- En JS/TS, los tipos del compilador de TypeScript para reconocer conexiones y objetos del pedido
  sin depender del nombre de la variable.
