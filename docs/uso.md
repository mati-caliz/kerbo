# Uso

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
`.npmrc`) sólo se revisan buscando secretos. Python recorre como ruff: también lo oculto, salvo sus
exclusiones por defecto (`.venv`, `venv`, `dist`, `site-packages` y compañía). Sale con `0` si no hay problemas, `1` si los hay y
`2` si algo no se pudo leer o la configuración es inválida.

`kerbo check`, `kerbo baseline` y el servidor MCP guardan lo que miden de cada archivo en una
caché por contenido, así la corrida siguiente sólo analiza lo que cambió (y lo que se compara entre
archivos, como los bloques duplicados, se recombina con lo guardado). Vive en la carpeta de caché
del usuario (`~/.cache/kerbo`, `~/Library/Caches/kerbo` o `%LOCALAPPDATA%\kerbo`; otra con
`KERBO_CACHE_DIR`), se comparte entre proyectos y clones, ocupa como mucho 256 MB (otro tope con
`max_megabytes` en la sección `[cache]` de `kerbo.toml`, o con `KERBO_CACHE_MAX_MB`, que manda
sobre el archivo) y se descarta sola cuando cambia el binario o la configuración que afecta a un
archivo. Dos corridas a la vez no se pisan: cada parte de la caché se escribe con un lock del
sistema y releyendo lo que dejó la otra. Si se rompe, se ignora.

Los mensajes salen en español o en inglés. Manda `--lang en` (o `es`), después `lang` en
`kerbo.toml` y después el locale del sistema (`LC_ALL`, `LC_MESSAGES`, `LANG`); sin nada de eso,
español. El idioma sólo cambia el texto para personas: los identificadores de reglas, los `id`
estables y la forma del JSON, de SARIF y de `--format agent` y `agent-json` son los mismos.

## Arreglar de una vez

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
contexto (`git apply --unidiff-zero`, también con CRLF). Como ruff, los arreglos que pueden cambiar lo
que hace el código (borrar una variable sin usar cuyo valor tiene efectos, `x == True` que pasa a `x`)
sólo se aplican con `--unsafe-fixes`; el "arreglar todo" del LSP tampoco los toma. `--since <referencia>` arregla sólo los
problemas de las líneas que cambiaron desde esa referencia de git. Una vuelta de arreglos que deja
errores de sintaxis nuevos no se acepta. Sale con `0` si todo quedó escrito, `1` si algún archivo
no se escribió porque el arreglo dejaba algo nuevo y `2` si algo no se pudo leer o escribir.

## Adoptarlo en un repo que ya existe

`kerbo baseline` guarda los problemas actuales en `kerbo-baseline.json` (commitealo). Desde ahí,
`kerbo check` sólo reporta los problemas nuevos y los que **empeoraron**: si una función de la
línea de base pasa de complejidad 14 a 16, vuelve a aparecer. Cada problema se identifica por
regla, archivo y función, no por número de línea, así que mover código no lo hace "nuevo".
`--no-baseline` muestra todo.

## Antitrampa

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
cuántos se ignoraron en el código. `squawk-ignore` vale, como en squawk, para su línea y la
siguiente (también al final de una línea o en un `/* */`, con el motivo después de otro `--`), y
`//nolint` de Go, con el alcance de golangci-lint. En `--format sarif` lo ignorado sale como
suprimido en el código (`inSource`), con su motivo, y GitHub lo muestra como descartado.
