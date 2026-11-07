# kerbo

Español · [English](README.en.md)

**Un solo linter para todo el repo.** kerbo revisa la calidad del código en Java,
TypeScript/JavaScript, Python, Go y Rust, más los scripts de shell, las migraciones de Postgres,
los Dockerfile y los docker compose. Es un único binario, tarda segundos y da los mismos
resultados que la herramienta que ya usás para cada cosa.

```console
$ kerbo check
src/billing/InvoiceService.java:88:5: cyclomatic-complexity La función `settle` tiene complejidad ciclomática 11 (máximo 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity La función `applyPromotions` tiene complejidad cognitiva 19 (máximo 15)
kerbo: 5810 archivos, 2 problemas, 2250 ms
```

## Por qué

- **Reemplaza a varias herramientas** sin cambiar el criterio. Cada parte se compara contra la
  original sobre repos reales grandes (django, next.js, kubernetes, terraform…) hasta dar los
  mismos avisos en el mismo archivo, línea y columna:

  | Revisa | Igual que |
  |---|---|
  | Python | ruff (`E4`, `E7`, `E9`, `F`) |
  | TypeScript y JavaScript | ESLint y typescript-eslint recommended |
  | Go | golangci-lint (errcheck, ineffassign, staticcheck, unused) |
  | Java | Checkstyle (`sun_checks`), PMD y varias inspecciones de IntelliJ |
  | Rust | clippy (largo, parámetros, complejidad y anidamiento) |
  | Shell | ShellCheck |
  | Migraciones de Postgres (SQL, Prisma, drizzle, Flyway, Alembic) | squawk |
  | Dockerfile | hadolint |
  | Secretos commiteados | gitleaks |

- **Las mismas métricas en todos los lenguajes**: largo de archivo y de función, complejidad
  ciclomática y cognitiva, parámetros, anidamiento, números mágicos y código duplicado.
- **Nada se afloja en silencio.** Cada excepción va en `kerbo.toml` con su motivo, y con `--diff`
  marca lo que un cambio agrega para esquivar un problema en vez de arreglarlo: un
  `eslint-disable`, un test salteado o borrado, un `any`.
- **Pensado para agentes de IA**: un hook para Claude Code que revisa cada archivo que el agente
  edita, un servidor MCP y una salida compacta que gasta pocos tokens.
- **Rápido**: analiza en paralelo y guarda una caché por contenido, así la segunda corrida sólo
  mira lo que cambió.

## Instalación

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh      # Linux, macOS y Git Bash en Windows
irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex           # PowerShell en Windows
brew install mati-caliz/tap/kerbo                                # Homebrew
cargo install kerbo --locked                                     # con Rust
```

El instalador baja el binario de los [releases](https://github.com/mati-caliz/kerbo/releases),
verifica el sha256 y lo deja en `~/.local/bin` (en Windows, en `%LOCALAPPDATA%\Programs\kerbo`).

## Uso

```sh
kerbo check                  # revisa el directorio actual
kerbo check --diff main      # sólo las líneas que cambiaron desde main
kerbo fix                    # aplica los arreglos mecánicos
kerbo baseline               # acepta lo que ya hay: desde ahora sólo se reporta lo nuevo
kerbo explain <regla>        # por qué existe una regla y cómo se arregla
```

Sale con `0` si no hay problemas, `1` si los hay y `2` si algo no se pudo leer. Sin configurar
nada usa el preset más exigente; para aflojarlo, un `kerbo.toml` en la raíz del repo:

```toml
preset = "recommended"            # strict (de fábrica), recommended o minimal
lang = "es"                       # mensajes en español

[[exceptions]]
paths = ["src/legacy/**"]
rule = "cognitive-complexity"
reason = "se reescribe en el primer trimestre"
```

En GitHub Actions, los problemas aparecen anotados en el pull request:

```yaml
- uses: mati-caliz/kerbo@v0.2.0
  with:
    args: --diff origin/${{ github.base_ref }}
```

Con Claude Code, agregá el hook en `.claude/settings.json` y kerbo le devuelve al agente lo que
encuentra en cada edición, para que lo corrija en la misma vuelta:

```json
{ "hooks": { "PostToolUse": [{ "matcher": "Edit|MultiEdit|Write", "hooks": [{ "type": "command", "command": "kerbo hook" }] }] } }
```

## Documentación

- [Uso](docs/uso.md): todas las opciones, la caché, `kerbo fix`, la línea de base y la antitrampa.
- [Configuración](docs/configuracion.md): `kerbo.toml`, los presets y GitHub Actions.
- [Agentes de IA y editores](docs/agentes-y-editores.md): el hook, el demonio, MCP y LSP.
- [Reglas](docs/reglas.md): las métricas comunes y sus umbrales por lenguaje.
- [Java](docs/java.md): Checkstyle, las inspecciones de IntelliJ y cómo lee las librerías.
- [Seguridad](docs/seguridad.md): inyecciones, secretos y dependencias vulnerables.
- [Paridad](docs/paridad.md): cada herramienta que reemplaza, regla por regla y medida contra
  qué repos.
- [Cómo está armado](docs/arquitectura.md) y cómo correr el gate de desarrollo.
- [Hoja de ruta](ROADMAP.md).

## Licencia

[Apache 2.0](LICENSE).
