# Configuración

## `kerbo.toml`

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
errcheck_exclude_functions = ["os.Remove"]  # como exclude-functions de errcheck
errcheck_check_blank = true       # como check-blank: también `_ = f()`
errcheck_check_type_assertions = true  # como check-type-assertions: también `x.(T)` sin `ok`

[java]                            # opcional: a quién imitan redundant-modifier y final-class
reference = "pmd"                 # checkstyle o pmd; si falta, se detecta (ver Java)
encoding = "ISO-8859-1"          # UTF-8, ISO-8859-1 o US-ASCII; si falta, la del pom.xml o build.gradle
```

| Preset | Archivo | Función | Ciclomática | Cognitiva | Parámetros |
|---|---|---|---|---|---|
| `strict` | los de [Reglas](reglas.md), por lenguaje | | | | |
| `recommended` | 750 | 150 | 15 | 20 | 6 |
| `minimal` | 1500 | 300 | 25 | 30 | 8 |

## GitHub Actions

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
