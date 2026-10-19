# kerbo

Analizador de calidad de código para **Java, TypeScript/JavaScript, Python y Go** en un solo
binario. Aplica las mismas reglas en todos los lenguajes, en segundos y con pocos megas de
memoria, y cuenta igual que la herramienta que ya usás en cada uno (ESLint, ruff, Checkstyle y
PMD, golangci-lint).

```console
$ kerbo check
src/billing/InvoiceService.java:88:5: cyclomatic-complexity La función `settle` tiene complejidad ciclomática 11 (máximo 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity La función `applyPromotions` tiene complejidad cognitiva 19 (máximo 15)
kerbo: 5810 archivos, 2 problemas, 2250 ms
```

## Instalación

Linux (x86_64, binario estático que corre en cualquier distro):

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

El instalador baja la última versión de los
[releases](https://github.com/mati-caliz/kerbo/releases), verifica el sha256 y deja `kerbo` en
`~/.local/bin` (otra carpeta con `KERBO_INSTALL_DIR`).

macOS, Windows y otras arquitecturas, con Rust instalado:

```sh
cargo install --git https://github.com/mati-caliz/kerbo --locked
```

## Uso

```sh
kerbo check                       # el directorio actual
kerbo check api web               # varias carpetas o archivos
kerbo check --language java api   # sólo una pila: typescript, python, java o go
kerbo check --diff main           # sólo lo que tocan las líneas cambiadas desde main
kerbo check --format sarif        # también json, agent; sarif lo leen GitHub y los editores
kerbo baseline                    # acepta lo que ya hay: desde ahora sólo se reporta lo nuevo
kerbo explain cognitive-complexity  # por qué existe una regla y cómo se arregla
```

Respeta `.gitignore` y saltea los archivos ocultos. Sale con `0` si no hay problemas, `1` si los
hay y `2` si algo no se pudo leer o la configuración es inválida.

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
| `suppression-added` | `eslint-disable`, `@ts-ignore`, `noqa`, `type: ignore`, `nolint`, `#nosec`, `@SuppressWarnings`, `NOPMD`, `CHECKSTYLE:OFF`, `istanbul ignore`, `NOSONAR`… |
| `test-disabled` | `it.skip`, `it.only`, `xit`, `describe.only`, `@Disabled`, `@Ignore`, `@pytest.mark.skip`, `t.Skip()`… |
| `test-removed` | tests borrados, comparando contra la referencia; si los tests sólo se movieron de archivo, no avisa |
| `type-escape` | `any` y `as unknown as` en TypeScript |

Todo se detecta sobre el árbol de sintaxis: un `eslint-disable` dentro de un string no cuenta. Si
una excepción es legítima, va en `kerbo.toml` con su `reason`, a la vista de todos.

### Para agentes de IA

- `kerbo check --format agent`: JSON compacto con tope, las trampas primero y después lo que más
  se pasa del límite, cada problema con un identificador estable y una pista de arreglo.
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
- **Servidor MCP** (`kerbo mcp`, por stdio) con dos herramientas, `kerbo_check` (carpetas, `diff`
  y lenguajes; devuelve la salida para agentes) y `kerbo_explain`. En Claude Code:
  `claude mcp add kerbo -- kerbo mcp`; en Cursor y otros clientes, un servidor stdio con el
  comando `kerbo` y el argumento `mcp`.

### En el editor

`kerbo lsp` es un servidor LSP: el editor le pasa el archivo mientras escribís y kerbo marca los
problemas al instante, sin esperar a que guardes, con el `kerbo.toml` y la línea de base del repo.

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

[[limits]]                        # un umbral para todos los lenguajes, o para uno
rule = "cyclomatic-complexity"
max = 12
language = "java"                 # opcional: typescript, python, java o go
reason = "el equipo acordó 12 para los servicios"

[[exceptions]]                    # por carpeta o archivo
paths = ["src/legacy/**"]
rule = "cognitive-complexity"     # sin `max`, la regla no corre ahí; sin `rule`, ninguna
reason = "se reescribe en el primer trimestre"

[[ignore]]
paths = ["generated/**"]
reason = "lo genera el compilador de protos"
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

| Regla | TypeScript / JavaScript | Python | Java | Go |
|---|---|---|---|---|
| `file-length` | 400 líneas de código | — | 500 líneas | 400 líneas |
| `function-length` | 120 líneas de código | — | 80 líneas desde la llave | 80 líneas internas |
| `cyclomatic-complexity` | 10 | 10 | 8 | 10 |
| `cognitive-complexity` | 15 | — | 15 | 15 |
| `parameter-count` | 4 | 6 (sin `self` ni `cls`) | 5 (sin `@Override`) | 4 |

Cada lenguaje cuenta como su herramienta de referencia: ESLint y sonarjs, ruff, Checkstyle y PMD,
cyclop, gocognit, funlen y revive. Esas diferencias de criterio no están documentadas en ningún
lado; cada una salió de comparar contra repos reales y quedó fijada en un test de
`src/metrics/tests.rs`. En los tests de TypeScript no se mide el largo de función, y en los de
Python no se cuentan los parámetros.

Esos son los umbrales del preset `strict`. Más reglas y la capa para agentes de IA están en la
[hoja de ruta](ROADMAP.md).

## Cómo está armado

Las métricas se escriben una sola vez sobre el árbol de [tree-sitter](https://tree-sitter.github.io/).
Lo propio de cada lenguaje vive en `src/profile/`: qué nodo es una función, una decisión o un
operador lógico, y cómo cuenta líneas la herramienta que se imita. Los archivos se analizan en
paralelo y de a uno, sin cargar el programa entero.

## Desarrollo

El gate (formato, clippy en modo estricto, techos de tamaño, tests con cobertura y `cargo-deny`)
es `scripts/verify-rust.sh` y corre en un contenedor, así que sólo hace falta Docker.

## Licencia

[Apache 2.0](LICENSE).
