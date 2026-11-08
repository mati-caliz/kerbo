# Paridad con otras herramientas

## Python contra ruff

En Python, kerbo hace las reglas del conjunto clásico de ruff (`E4`, `E7`, `E9` y `F`: pyflakes y
los errores de pycodestyle) con el mismo código que ruff, en minúscula: `kerbo explain f401`,
`--max f841=0` o `f401 = "off"` en `kerbo.toml`. Respeta `# noqa`, `# noqa: F401` y
`# ruff: noqa` como ruff (en strings de varias líneas y líneas continuadas con `\`, el `noqa` va
en la última). `minimal` deja sólo las `F`.

| Reglas | Qué marca |
|---|---|
| `f401` | imports sin usar (se arregla solo, salvo en `__init__.py`) |
| `f811`, `f402` | redefiniciones de algo nunca usado y variables de un `for` que pisan un import |
| `f821`, `f405`, `f822`, `f823` | nombres no definidos (con los alcances de módulo, clase, función y comprensión, las anotaciones entre comillas y `from __future__ import annotations`), los que pueden venir de un `import *`, los de `__all__` y los locales usados antes de asignarse |
| `f841`, `f842` | variables locales asignadas o anotadas y nunca usadas (el `as` de un `except` se arregla solo) |
| `f403`, `f404`, `f406` | `import *`, `from __future__` tarde y `import *` fuera del módulo |
| `f501`–`f509`, `f521`–`f525` | formatos con `%` y `.format()` que no cierran con sus argumentos |
| `f541` | f-strings sin reemplazos (se arregla solo) |
| `f601`, `f602` | claves repetidas en un diccionario |
| `f631`, `f632`, `f633`, `f634` | `assert` e `if` sobre una tupla, `is` contra un literal (se arregla solo) y `print >>` |
| `f701`, `f702`, `f704`, `f706`, `f707`, `f722`, `f901` | `break`, `continue`, `yield`, `await` y `return` fuera de lugar, `except:` antes de otros, anotaciones entre comillas que no compilan y `raise NotImplemented` (se arregla solo) |
| `e401`, `e402` | varios módulos en un `import` e imports después de otro código |
| `e701`, `e702`, `e703` | varias sentencias en una línea (el `;` final se arregla solo) |
| `e711`, `e712`, `e713`, `e714`, `e721` | `== None`, `== True`, `not x in y`, `not x is y` (se arreglan solos) y comparaciones de tipos con `==` |
| `e722`, `e731`, `e741`, `e742`, `e743` | `except:` pelado, lambdas asignadas y nombres `l`, `I` y `O` |

Paridad, contra ruff 0.17 con `--select E4,E7,E9,F`: cero diferencias por archivo, línea, columna y
regla en django, fastapi, requests, flask, pydantic, sqlalchemy, ipython, ansible, sympy y la
biblioteca estándar de CPython (unos 11.000 archivos y 15.900 avisos). Las únicas que quedan son
las de archivos que tree-sitter-python no parsea (ahí kerbo no corre las reglas de alcance) y E902,
que es un error de lectura y no una regla.

## TypeScript y JavaScript contra ESLint

En TypeScript y JavaScript, kerbo hace las reglas de `@eslint/js` recommended y de
`typescript-eslint` recommended que no piden tipos, con el nombre de ESLint sin el prefijo del
plugin: `kerbo explain no-unused-vars`, `--max no-explicit-any=0` o `no-var = "off"`. Corren en los
dos lenguajes como en un `eslint.config.js` con `js.configs.recommended` y
`tseslint.configs.recommended`; en los `.ts` no corre `no-dupe-keys`, que ya revisa el compilador.
`no-var` y `prefer-const` van también en JavaScript y `no-non-null-assertion` (de `strict`) entra
en el preset `strict`. `recommended` la apaga y `minimal` deja sólo las que ESLint clasifica como
problemas. Los `eslint-disable` no silencian nada: el aviso sale y el comentario lo marca la
antitrampa.

| Reglas | Qué marca |
|---|---|
| `no-unused-vars` | variables, funciones, clases, tipos, imports y parámetros sin usar, con los alcances de `@typescript-eslint/scope-manager` (tipos y valores por separado, `typeof`, JSX contra `React`, declaraciones ambientales y `.d.ts`) |
| `prefer-const`, `no-var` | `let` que nunca se reasigna y `var`; los dos se arreglan solos cuando ESLint los arregla |
| `no-unused-expressions` | expresiones sueltas que no hacen nada |
| `no-require-imports` | `require()` sin un `require` propio e `import x = require()` |
| `no-explicit-any`, `no-non-null-assertion`, `ban-ts-comment` | `any`, el `!` de no nulo y `@ts-ignore`, `@ts-nocheck` o `@ts-expect-error` sin motivo |
| `no-useless-escape` | escapes que no hacen falta en strings, templates y expresiones regulares (se arregla solo) |
| `prefer-rest-params`, `no-prototype-builtins`, `no-this-alias`, `no-empty-object-type` | `arguments`, `obj.hasOwnProperty()`, `const self = this` y el tipo `{}` |
| `no-empty`, `no-cond-assign`, `no-case-declarations` | bloques vacíos, asignaciones en condiciones y declaraciones sueltas en un `case` |
| `no-debugger`, `no-duplicate-case`, `no-dupe-else-if`, `no-dupe-keys` | `debugger` (se arregla solo), casos, ramas y claves repetidas |

Paridad, contra ESLint 10.14 y typescript-eslint 8.72: ninguna diferencia de más por archivo,
línea, columna y regla en next.js, react, VS Code, zod y express (42.488 avisos en unos 30.000
archivos). Faltan 587 avisos, todos de archivos que tree-sitter no entiende (varianza `in`/`out`,
`let as`, `export type *`, escapes en tipos de template) o de `.js` con argumentos de tipo en
llamadas, que TypeScript lee como comparaciones.

## Shell contra ShellCheck

Los `.sh`, los `.bash` y los archivos sin extensión con shebang de `sh`, `bash`, `dash` o busybox
se revisan contra [ShellCheck](https://www.shellcheck.net/), con su código como nombre y en
minúscula: `kerbo explain SC2086`, `--max sc2086=0` o `sc2034 = "off"` en `kerbo.toml`. Son las 22
reglas que más aparecen, las 64 de lo que bash agrega en un script de sh (`sc3001` a `sc3067`), 55
chequeos de comandos (`tr`, `grep`, `expr`, `find`, `rm`, `sudo`, `return`, `alias`, `read` y otros),
24 de `source`, pipelines y palabras (`ls | grep`, `"x$@"`, `$*`, comillas que descomillan y otras) y
46 de condiciones, `case`, subshells, redirecciones y asignaciones, 4 sobre el flujo de variables, 43 de comparaciones y aritmética, 35 de `for`, `find -exec`, `exec`, `break`, funciones y shebang, 36 de nombres de comando, pipes, redirecciones y arreglos, 19 del detalle de `printf`, `getopts`, `time` y las declaraciones 10 de código inalcanzable y el tipo de cada variable y 4 sueltas (`sc1003`, `sc1007`, `sc2251`, `sc2316`), que completan los chequeos por defecto de ShellCheck salvo lo que tree-sitter-bash no deja ver; la severidad es la de ShellCheck, `recommended` apaga las de estilo y
`minimal`, también las informativas. Las [reglas universales](reglas.md) no corren en
shell, salvo la antitrampa.

| Reglas | Qué marca |
|---|---|
| `sc2086` | expansiones sin comillas que pueden partirse o expandir comodines, con el valor de cada variable seguido por ramas, loops y funciones como ShellCheck (se arregla solo) |
| `sc2046`, `sc2206`, `sc2207`, `sc2068` | `$(…)` y `${arr[@]}` sin comillas, y arreglos armados con palabras que se parten |
| `sc2006`, `sc2005`, `sc2116` | backticks (se arregla solo), `echo $(cmd)` y `$(echo …)` |
| `sc2034` | variables asignadas y nunca usadas |
| `sc2155` | `local x=$(cmd)`, que esconde el código de salida |
| `sc2162`, `sc2164` | `read` sin `-r` y `cd` sin `\|\| exit` (los dos se arreglan solos) |
| `sc2004`, `sc2181`, `sc2015`, `sc2166` | `$` de más en aritmética, `[ $? -ne 0 ]`, `a && b \|\| c` y `-a`/`-o` en `[ ]` |
| `sc2016`, `sc2059`, `sc2064`, `sc2069`, `sc2223` | expansiones entre comillas simples, variables en el formato de `printf`, `trap` con comillas dobles, redirecciones al revés y `: ${x:=…}` sin comillas |
| `sc2148` | un script sin shebang ni `# shellcheck shell=` |
| `sc2003`, `sc2018` a `sc2029`, `sc2060` a `sc2063`, `sc2114` a `sc2313` | lo de `Checks/Commands.hs` de ShellCheck: `tr a-z`, globs sin comillas en `find -name`, `grep` y `unset`, `expr`, `rm -rf "$dir/"`, `sudo` con redirecciones, `return`/`exit` fuera de 0-255, `export $x`, `alias` con `$1`, `egrep`, `mv` sin destino y el resto |
| `sc1090`, `sc1091`, `sc2007` a `sc2013`, `sc2026`, `sc2027`, `sc2038`, `sc2048`, `sc2070`, `sc2084`, `sc2088`, `sc2091`, `sc2092`, `sc2119`, `sc2120`, `sc2124` a `sc2126`, `sc2140`, `sc2145` | los `source` que no se siguen, `ps \| grep`, `ls \| grep`, `find \| xargs` sin `-0`, `for x in $(cat f)`, `$*` sin comillas, `"texto $@"`, arreglos y globs asignados a una variable, `[ -n $x ]`, `"a"$x"b"`, `~` entre comillas, `$(cmd)` como comando y funciones que leen `$1` sin que nadie se los pase |
| `sc2050`, `sc2076` a `sc2078`, `sc2143`, `sc2144`, `sc2157` a `sc2161`, `sc2193`, `sc2198` a `sc2203`, `sc2208`, `sc2245`, `sc2255`, `sc2268` | condiciones que no deciden nada o no hacen lo que parece: `[ n -le 4 ]`, `[ $a=3 ]`, `[ false ]`, `[ -n "$a " ]`, globs, llaves y arreglos como operandos, `=~ "x.*"`, `[ -n "$(grep x)" ]` y el prefijo `x` |
| `sc2001`, `sc2035`, `sc2037`, `sc2044`, `sc2045`, `sc2079`, `sc2094`, `sc2103`, `sc2129`, `sc2141`, `sc2194`, `sc2195`, `sc2209`, `sc2221`, `sc2222`, `sc2231`, `sc2233` a `sc2235`, `sc2269`, `sc2295`, `sc2321`, `sc2327`, `sc2328` | `echo "$x" \| sed`, `rm *`, `A=ls -l`, `for` sobre `ls` o `find`, leer y escribir el mismo archivo, `cd` de vuelta, varios `>>` seguidos, `IFS="\n"`, patrones de `case` inalcanzables, `( [ a ] )`, `x=$x`, `${a%$b}`, `a[$((i))]=` y `$(cmd > f)` |
| `sc2017`, `sc2049`, `sc2053` a `sc2058`, `sc2065`, `sc2071` a `sc2074`, `sc2080`, `sc2081`, `sc2099`, `sc2100`, `sc2107` a `sc2110`, `sc2122`, `sc2170`, `sc2171`, `sc2204`, `sc2205`, `sc2212`, `sc2252`, `sc2254`, `sc2257`, `sc2265`, `sc2266`, `sc2283` a `sc2285`, `sc2309`, `sc2322` a `sc2324`, `sc2330`, `sc2331`, `sc2333`, `sc2334` | comparaciones que no comparan lo que parece: `[[ $a > 3 ]]`, `[ a && b ]`, `[[ a -o b ]]`, `[ $a = *.txt ]`, `[ $a != x ] \|\| [ $a != y ]`, `test 1 > 2`, `a = 1`, `( -e f )`, `a=(1, 2)`, `i=i+1`, `a+=1`, `08` en aritmética y paréntesis de más |
| `sc2000`, `sc2014`, `sc2032`, `sc2033`, `sc2036`, `sc2041` a `sc2043`, `sc2066`, `sc2067`, `sc2083`, `sc2087`, `sc2093`, `sc2095` a `sc2098`, `sc2101`, `sc2102`, `sc2104` a `sc2106`, `sc2111` a `sc2113`, `sc2118`, `sc2123`, `sc2127`, `sc2147`, `sc2165`, `sc2167`, `sc2187`, `sc2239`, `sc2246`, `sc2258` | el flujo del script: `for` que corre una vez, `find -exec` sin terminar, código después de `exec`, `break` afuera de un loop o en un subshell, `sudo` con una función, `ssh` en un `while read`, `a=1 echo $a`, `[:digit:]`, `PATH=/x`, `function` en sh, el shebang |
| `sc2188` a `sc2192`, `sc2210`, `sc2211`, `sc2215` a `sc2217`, `sc2238`, `sc2247`, `sc2256`, `sc2259` a `sc2264`, `sc2270` a `sc2282`, `sc2286` a `sc2289` | la forma del comando: `> file` solo, `< f \| cmd`, `foo \| rm`, `a > f \| b`, `a > f > g`, `foo > 2`, `foo > grep`, `*.sh` o `-e` como comando, `$a=1`, `1=a`, `=====`, `foo/`, `ls() { ls; }`, alias usados en su misma unidad, `$"(ls)"` y `a=(1=b)` |
| `sc2025`, `sc2051`, `sc2175` a `sc2177`, `sc2180`, `sc2182`, `sc2183`, `sc2213`, `sc2214`, `sc2220`, `sc2290`, `sc2291`, `sc2302`, `sc2303`, `sc2318`, `sc2325`, `sc2326`, `sc2332` | el detalle de un comando: formatos de `printf` con otra cantidad de argumentos, el `case` de `getopts`, `export a =b`, `declare a=1 b=$a`, `{1..$n}`, `time a \| b` en sh, `echo a    b`, `for i in "${a[@]}"` con `${a[$i]}`, `a \| ! b` y `[ ! -a f ]` |
| `sc2089`, `sc2090`, `sc2128`, `sc2178`, `sc2179`, `sc2218`, `sc2317`, `sc2319`, `sc2320`, `sc2329` | lo que sale del flujo: funciones que nunca se llaman o que se llaman antes de definirse, código después de un `exit`, arreglos usados como texto, comillas guardadas en una variable y `$?` que ya no es el del comando que se quería mirar |
| `sc2030`, `sc2031`, `sc2153`, `sc2154` | lo que se asigna adentro de un subshell y se lee afuera, y variables que nada asigna en el archivo (las de mayúsculas, sólo si hay una parecida) |
| `sc3001` a `sc3067` | en un script de `sh`, `dash` o busybox, lo que sólo tiene bash: `[[ ]]`, `local`, arreglos, `${x/a/b}`, `echo -n`, `&>`, `source`, `set -o pipefail` y el resto de `checkBashisms` de ShellCheck, con las excepciones de dash y busybox |

`# shellcheck disable=` vale como en ShellCheck: antes del primer comando, para el archivo entero;
antes de un comando compuesto, para todo el bloque; acepta rangos (`SC2000-SC2099`) y `all`.

`.shellcheckrc` (o `shellcheckrc`) también se lee: el primero que aparece subiendo desde el script, sin
salir del repositorio. `disable=` manda los avisos a la lista de suprimidos con el nombre del archivo como
motivo, y `shell=` elige el dialecto si el script no trae su directiva. En la antitrampa, un código nuevo
en un `disable=` cuenta como una excepción agregada.

Paridad, contra ShellCheck 0.11: cero diferencias por archivo, línea, columna y regla en 1.752
scripts de kubernetes, moby, nvm, rustup, ohmyzsh, acme.sh, bash-it, dokku y otros (1.680 avisos).
Con esos repos actualizados (1.834 scripts, leyendo los `.shellcheckrc`): 2.033 avisos idénticos y 87 de más, todos
`sc2016` de un `nvm.sh` nuevo que tree-sitter-bash lee mal; sin las directivas `# shellcheck`,
3.747 idénticos y ninguno de más. Los archivos que tree-sitter-bash no parsea se
saltean.

## Go contra golangci-lint

En Go, kerbo hace reglas del conjunto por defecto de golangci-lint con el mismo nombre:
`kerbo explain errcheck`, `--max ineffassign=0` o `qf1003 = "off"` en `kerbo.toml`. `errcheck`
marca el `error` que devuelve una llamada y nadie revisa, con las exclusiones de errcheck; los
tipos los saca el chequeo de tipos del propio Go con un helper que kerbo compila una vez con el
`go` del sistema (sin `go`, el escaneo falla con el motivo, o se apaga con `errcheck = "off"`). `ineffassign` marca una asignación que nadie lee antes de que se
pise o termine la función, siguiendo ramas, loops, `switch` con `fallthrough`, `select`, `goto` y
etiquetas; no cuentan las variables que captura un closure ni aquellas cuya dirección se toma. De
staticcheck: `st1005`, mensajes de error con mayúscula o puntuación al final (con las constantes de
todo el paquete y las mismas excepciones), `qf1003`, cadenas de `if` que comparan siempre lo mismo
y podrían ser un `switch`, y `qf1004`, `strings.Replace` y compañía con `-1` de límite.

Como golangci-lint, saltea los archivos generados, los que no compilan en la plataforma (sufijos
como `_windows.go`, `//go:build` y `// +build`) y lo que `./...` no lista desde el `go.mod`
(`testdata`, `vendor` y las carpetas que empiezan con `_` o `.`). La plataforma es la de `GOOS` y
`GOARCH` o la máquina donde corre kerbo; la sección `[go]` de `kerbo.toml` la fija y suma build
tags.

`//nolint` vale como en golangci-lint: al final de una línea, para esa línea; en su propia línea,
para lo que empieza justo abajo en la misma columna (una función, un `if`, el archivo entero si
está antes de `package`). Sin lista o con `all` silencia todas las reglas, y el motivo de
`//nolint:ineffassign // motivo` aparece en el reporte.

Paridad, contra golangci-lint 2.15 en linux/amd64: cero diferencias por archivo, línea y columna
en golang/tools, terraform, hugo, caddy, consul, prometheus, etcd, syncthing y otros (1.381
avisos), en errcheck 10.754, en `qf1008` y `qf1012` 1.153 y en `sa1019` (uso de lo deprecado)
592, `qf1001` 136, `stdmethods` de govet 28 y la cola chica de staticcheck (`st1012`, `qf1011`,
`st1023`, `qf1002`, `s1005`, `sa9003`, `qf1006`, `st1019`, `sa1029`, `sa9004`, `qf1009`, `st1013`,
`s1000`, `st1017`, `st1006`, `s1008`, `s1002` y `s1034`) 204 y en `unused` (código que nadie usa) 249, sin
ninguno de más: el 99 % de lo que marca golangci-lint por defecto. Suma otras 42 reglas chicas de
staticcheck (`s1009`, `s1011`, `s1017`, `s1038`, `sa1006`, `sa2001`, `sa4004`, `sa4016`, `sa6002`,
`st1008`, `qf1007` y más) que dan lo mismo que golangci-lint en sus casos de prueba y en los 26 avisos del corpus. Lo demás de
govet y de staticcheck sigue en golangci-lint.

## Migraciones y contenedores

kerbo también revisa lo que se despliega junto al código: las migraciones de Postgres, los
Dockerfile y los archivos de docker compose.

**Migraciones.** Son migraciones los `.sql` dentro de una carpeta `migrations`, `migration` o
`drizzle` (Prisma, drizzle, sqlx, golang-migrate) y los que siguen el nombre de Flyway
(`V3__agrega_indice.sql`, `R__vistas.sql`); el resto de los `.sql` (semillas, dumps) no se mira.
En Alembic se lee el `upgrade()` de cada revisión: cada `op.*` y `batch_op.*` se convierte en el
SQL que Alembic generaría para Postgres (tipos de SQLAlchemy, `server_default`, claves foráneas, la
clave primaria entera como `SERIAL`), junto con el de `op.execute(...)`, y todo pasa por las mismas
reglas que un `.sql`, con el aviso en la línea de la llamada. Se siguen los `for` sobre listas, las
funciones auxiliares del mismo archivo y las constantes del módulo, `autocommit_block()` sale de la
transacción, y las ramas de otro dialecto (`if dialect == "mysql"`) y los `except` se saltean. Las reglas son las de
[squawk](https://squawk.sh), con sus nombres y su criterio, suponiendo como él con
`--assume-in-transaction` que cada archivo corre en una transacción (así lo hacen Prisma,
drizzle, Flyway y Alembic en Postgres): un índice sobre una tabla creada en la misma migración no
se marca.

| Regla | Qué marca |
|---|---|
| `require-concurrent-index-creation` | `CREATE INDEX` sin `CONCURRENTLY` sobre una tabla que ya existía |
| `require-concurrent-index-deletion` | `DROP INDEX` sin `CONCURRENTLY` |
| `adding-required-field` | `ADD COLUMN ... NOT NULL` sin `DEFAULT` ni columna generada |
| `adding-not-nullable-field` | `SET NOT NULL` sin un `CHECK (columna IS NOT NULL) NOT VALID` validado antes |
| `changing-column-type` | `ALTER COLUMN ... TYPE` (también en tipos compuestos) |
| `renaming-column`, `renaming-table` | renombres de columnas, atributos y tablas |
| `ban-drop-column`, `ban-drop-table` | borrados de columnas, atributos y tablas |
| `require-lock-timeout`, `require-statement-timeout` | la primera operación que toma locks sin un `SET lock_timeout` o `statement_timeout` antes |
| `constraint-missing-not-valid`, `adding-foreign-key-constraint` | restricciones y claves foráneas sin `NOT VALID`, o validadas en la misma transacción |
| `adding-serial-primary-key-field`, `disallowed-unique-constraint` | `PRIMARY KEY` y `UNIQUE` sin `USING INDEX` sobre una tabla que ya existía |
| `adding-field-with-default` | una columna con un default volátil (`random()`, `gen_random_uuid()`) o generada |
| `prefer-bigint-over-int`, `prefer-bigint-over-smallint`, `prefer-identity`, `prefer-text-field`, `prefer-timestamp-tz`, `ban-char-field` | `int`, `smallint`, `serial`, `varchar(n)`, `timestamp` sin zona y `char(n)` en columnas nuevas o cambiadas |
| `ban-drop-constraint`, `ban-drop-default`, `ban-drop-not-null`, `ban-drop-generated-expression` | borrados de restricciones, defaults, `NOT NULL` y expresiones generadas |
| `ban-drop-type`, `ban-drop-function`, `ban-drop-sequence`, `ban-drop-view`, `ban-drop-schema`, `ban-drop-domain`, `ban-drop-extension`, `ban-drop-database` | borrados de objetos que el código puede usar |
| `renaming-object`, `ban-set-schema` | renombres de restricciones, índices, tipos, vistas y funciones, y objetos movidos a otro esquema |
| `require-enum-value-ordering`, `ban-alter-identity`, `ban-create-domain-with-constraint`, `ban-alter-domain-with-add-constraint` | valores de enum sin posición, identidades cambiadas y dominios con restricciones |
| `require-concurrent-reindex`, `require-concurrent-partition-detach`, `ban-truncate-cascade` | `REINDEX` y `DETACH PARTITION` sin `CONCURRENTLY`, y `TRUNCATE ... CASCADE` |
| `transaction-nesting`, `ban-uncommitted-transaction`, `ban-concurrent-index-creation-in-transaction`, `prefer-robust-stmts` | `BEGIN`/`COMMIT` dentro de la transacción de la herramienta, transacciones sin cerrar, `CONCURRENTLY` en una transacción y sentencias sin `IF [NOT] EXISTS` fuera de ella |
| `identifier-too-long`, `ban-duplicate-column-assignments` | nombres de más de 63 bytes y columnas repetidas en una tabla, un `INSERT`, un `SET` o una vista |

Es el conjunto por defecto de squawk 2.70 entero, salvo `prefer-repack`, que squawk sólo corre con
Postgres 19. `recommended` apaga las de preferencia de tipos (`prefer-*` y `ban-char-field`) y
`minimal`, también los timeouts. Paridad, contra squawk 2.70 con `--assume-in-transaction`: los
9.612 avisos de squawk, ninguno de más, en 1.858 migraciones de lobe-chat (drizzle), documenso,
trigger.dev y cal.com (Prisma), conductor (Flyway) y awesome-compose. En Alembic, contra squawk sobre
el SQL que genera el propio Alembic en modo offline (300 revisiones de airflow, superset, mlflow y
redash; las otras 349 importan la aplicación o consultan la base y no se pueden convertir sin
levantarla): 297 idénticas por regla y cantidad, ninguna con avisos de menos, y dos con avisos de
más en caminos que el modo offline no recorre (`is_offline_mode()` y un `try` que en offline se
corta).

**Dockerfile.** `Dockerfile`, `Containerfile`, `Dockerfile.*` y `*.Dockerfile`, contra
[hadolint](https://github.com/hadolint/hadolint): todas las reglas DL que hadolint trae prendidas
(59), con su código como nombre (`kerbo explain DL3008`). El shell de cada `RUN` se lee con un
parser propio que separa los comandos como ShellCheck. Quedan afuera las reglas de ShellCheck
(`SC*`), que van con la etapa 10, y las que hadolint trae apagadas o sólo corren con su
configuración (DL1001, DL3026, DL3049 a DL3058). `.hadolint.yaml` no se lee: una regla se apaga en
`kerbo.toml`, con su motivo. `recommended` apaga las de nivel `info` de hadolint y `minimal` deja
sólo las de nivel `error`. Paridad, contra hadolint 2.15.1 con su configuración por defecto: los
408 avisos de hadolint, ninguno de más, en 169 Dockerfiles de awesome-compose, docker-stacks,
airflow, superset, mlflow, nextcloud/docker, mastodon, sentry self-hosted, grafana y redash.

**docker compose.** `compose.yaml`, `docker-compose.yml` y sus variantes (`docker-compose.prod.yml`),
con anclas y `<<:` resueltos. No hay herramienta de referencia: el criterio es propio.

| Regla | Qué marca |
|---|---|
| `compose-memory-limit` | un servicio con `image` o `build` sin `deploy.resources.limits.memory` ni `mem_limit`; los que usan `extends` y los fragmentos de un override se saltean |
| `compose-healthcheck-start-period` | un `healthcheck` activo sin `start_period` |
| `compose-healthcheck-interval` | un `interval` de menos de 5 s o más de 5 min, o un `timeout` igual o mayor que el intervalo |
| `compose-cpu-limit` | un servicio con `image` o `build` sin `deploy.resources.limits.cpus` ni `cpus` |
| `compose-restart` | un servicio sin `restart` ni `deploy.restart_policy` (`restart: "no"` vale para una tarea que corre una vez) |
| `compose-image-tag` | una imagen sin tag o con `latest`; un digest o una variable no cuentan |
| `compose-depends-on-healthy` | un `depends_on` hacia un servicio con healthcheck que no pide `condition: service_healthy` |

Cada una se apaga como cualquier otra regla, con `[[limits]]` sin `max` y su motivo; el `max` no
aplica. Con `--language sql` o `--language docker` se analiza sólo eso.
