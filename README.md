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
kerbo check --format json         # para CI o para un agente
```

Respeta `.gitignore` y saltea los archivos ocultos. Sale con `0` si no hay problemas, `1` si los
hay y `2` si algún archivo no se pudo leer.

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

Por ahora los umbrales son fijos (el preset estricto). La configuración por repo, más reglas y la
capa para agentes de IA están en la [hoja de ruta](ROADMAP.md).

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
