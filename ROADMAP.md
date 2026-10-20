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
- **No se reimplementan compiladores ni runners de tests.** Los tipos de TypeScript vienen del
  compilador; los tests corren con el runner de cada lenguaje, pero sólo los que afecta el cambio.

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
  `nolint`, `@SuppressWarnings`; `#[allow]` llega con Rust), afloja un umbral en una
  configuración, o borra, saltea o vacía un test.
- `--format agent`: JSON compacto, priorizado y con tope, e identificadores estables por
  problema (regla + símbolo + contexto, no número de línea).
- `kerbo explain <regla>`: por qué existe, ejemplo malo y bueno, cómo se arregla.
- **Modo instantáneo** sobre los archivos recién tocados (meta: menos de 100 ms), un hook listo
  para Claude Code y un **servidor MCP**.

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

### 5. Más reglas universales y Rust

- Anidamiento (hecho para TypeScript y Go, con paridad contra `max-depth` de ESLint y
  `max-control-nesting` de revive; Java mide `if`, `for` y `try` por separado y va con las reglas
  de Checkstyle).
- Números mágicos (hecho en los cuatro lenguajes, con paridad contra `no-magic-numbers` con las
  opciones del estándar, `MagicNumber` de Checkstyle, mnd y `PLR2004` de ruff).
- Nombres cortos (hecho en los cuatro lenguajes, con paridad contra `id-length` de ESLint,
  `LocalVariableName` de Checkstyle, varnamelen y los nombres ambiguos de ruff).
- Bloques duplicados y comentarios que narran lo obvio, cada una con paridad contra la
  herramienta que ya la tiene en cada lenguaje.
- **Rust** como quinto lenguaje (contra clippy), así kerbo se analiza a sí mismo.

### 6. Distribución

- Binarios para macOS (x86 y ARM), Windows y Linux ARM, armados sólo al publicar una versión.
- `cargo install kerbo` desde crates.io, y Homebrew.
- Mensajes, README y sitio en inglés además de español.

### 7. Caché y rendimiento

- Caché por contenido de archivo, comprimida y con tope de tamaño: sólo se analiza lo que
  cambió. Es lo que hace posible el modo instantáneo en repos grandes.
- Una sola pasada por función para todas las métricas, en lugar de una por métrica.

### 8. Java sintáctico

- Las reglas de forma de Checkstyle y PMD, con paridad medida. Es el hueco más grande: no hay un
  linter rápido de Java.
- Las que tienen un arreglo mecánico (imports sin usar, `final` que falta, modificadores
  redundantes) lo traen como acción rápida del LSP y como parche en `--format agent`.

### 9. Flujo de datos sin compilar

- Lo de SpotBugs sobre el código fuente: nulos, recursos sin cerrar, excepciones tragadas.
- Contaminación de entradas hacia SQL, comandos, rutas y HTML, en todos los lenguajes.

### 10. Go, Python y TypeScript completos

- Cada lenguaje contra su herramienta (golangci-lint, ruff, ESLint), regla por regla.

### 11. Tests inteligentes, dependencias y secretos

- Correr sólo los tests que afecta el cambio y reusar resultados, con el runner de cada lenguaje.
- Vulnerabilidades de dependencias con OSV y secretos versionados.

### 12. Formateo propio

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
