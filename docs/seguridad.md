# Seguridad

## Datos del usuario sin limpiar

Y de seguridad, lo que llega del usuario sin limpiar a una consulta (`sql-injection`), a un
comando (`command-injection`), a una ruta de archivo (`path-traversal`) o a la respuesta HTML
(`xss`). Las fuentes son los
métodos de `HttpServletRequest` que devuelven lo que mandó el usuario (`getParameter`,
`getHeader`, `getCookies`, `getQueryString`...) y los parámetros con `@RequestParam`,
`@PathVariable`, `@RequestBody` y el resto de las anotaciones web de Spring. Los sumideros son
`Statement`, `Connection`, `JdbcTemplate`, `EntityManager` y `JdbcClient` (en una consulta sólo
cuenta el texto: los argumentos con `?` son seguros), `Runtime.exec` y `ProcessBuilder`, y
`File`, `FileInputStream`, `FileOutputStream`, `FileReader`, `FileWriter`, `RandomAccessFile`,
`Paths.get` y `Path.of`, y lo que se escribe con `response.getWriter()` o `getOutputStream()`
(encadenado o guardado en una variable). Un escape de HTML (`encodeForHTML` de ESAPI,
`HtmlUtils.htmlEscape`, `escapeHtml4`, `Encode.forHtml`...) protege la respuesta pero no una
consulta ni una ruta: el dato escapado se sigue aparte. Todo es tabla de datos en
`src/lint/java_taint/tables.rs`.

Cada método se recorre como un intérprete: sabe qué variables son constantes y decide con ellas los
`if`, los `switch` y los ternarios (`(7 * 42) - 86 > 200 ? "fijo" : dato` toma siempre la misma
rama), y sigue posición por posición las listas y clave por clave los mapas que arma (sacar
`lista.get(1)` cuando el dato entró en la 0 no es contaminarse). Entre métodos y entre archivos,
cada uno deja un resumen (qué devuelve según sus parámetros y qué llega a cada sumidero) y el
índice los cruza: una llamada va a los métodos de su tipo, a los que hereda y a sus
implementaciones; un tipo del mismo archivo tapa a los homónimos de otros. Lo que devuelve un
método abstracto de una interfaz que nadie implementa en el código (un repositorio de Spring Data)
sale de la base, no de sus argumentos. Una llamada a una librería devuelve lo que traen el receptor
y los argumentos, salvo las que dan un número, un booleano o un UUID (`parseInt`, `length`,
`equals`, `UUID.fromString`...).

## Python

En Python, las mismas cuatro reglas con el mismo motor. Las fuentes son `request.args`, `.form`,
`.cookies`, `.headers`, `.values`, `.files`, `.json`, `.get_json()` y compañía de Flask, Django
(`GET`, `POST`, `COOKIES`, `META`, `body`) y Pyramid (`params`, `matchdict`), los parámetros de una
función de ruta (`@app.route`, `@router.get`...) y el `event` de un handler de Lambda. Los
sumideros: `.execute`, `.executemany` y `.executescript`, `text()` y los métodos de un `Query` de
SQLAlchemy que reciben SQL en texto; `os.system`, `os.popen` y `os.exec*`/`spawn*` siempre, y
`subprocess.*` sólo con `shell=True` o si lo que corre es una shell con `-c` (sin shell, un texto
armado no se interpreta); `open`, `codecs.open`, `os.remove`, `os.path.exists`, `shutil.*`,
`send_file` y los métodos de un objeto de `pathlib`; y el cuerpo de la respuesta: lo que devuelve
una vista de Flask (no si devuelve un diccionario o usa `jsonify`; en FastAPI es JSON),
`make_response` y `render_template_string`. Un `if '..' in ruta: return` valida la ruta; un escape
de HTML (`html.escape`, `markupsafe.escape`, `render_template` o una función propia que se llame
"escape…html"), `shlex.quote` y `secure_filename` limpian para su sumidero; lo que arma una
sentencia de SQLAlchemy (`select`, `insert(...).values(...)`) va con parámetros ligados. Entre
módulos, las llamadas se resuelven por los imports (también los relativos y los de adentro de una
función), las funciones y clases que se ven desde cada una, `self` y la clase con que se creó una
variable. `os.name` se toma como `"posix"`: es donde corre el servidor.

## JavaScript y TypeScript

En JavaScript y TypeScript, las mismas cuatro reglas. Sin tipos, las fuentes y los sumideros se
reconocen por el nombre: el pedido (`req`, `request`, `ctx`) y sus `query`, `params`, `body`,
`cookies`, `headers`, `url` y compañía; el `event` de un handler de Lambda (`(event, context)`, o
un único `event` en una función que se llama `handler`); y en el navegador, `location.hash`,
`location.search`, `document.URL`, `document.referrer` y `window.name`. Los sumideros: `query`,
`execute`, `raw` y `$queryRawUnsafe` sobre una conexión (`db`, `conn`, `pool`, `client`, `knex`,
`prisma`...); `child_process.exec` siempre y `spawn`/`execFile` con `{ shell: true }` o con el programa elegido por el usuario;
`fs.*` y `res.sendFile`; y para HTML, `res.send`, `res.write` y `res.end` de Express y Node (no
los de Fastify, que manda texto plano, ni si la función declara otro tipo con `res.type('json')`
o `writeHead(200, { 'content-type': 'application/json' })`), `innerHTML`, `document.write`,
`insertAdjacentHTML`, `dangerouslySetInnerHTML`, jQuery (`$("<b>" + x)`, `.html(x)`) y la
respuesta de un handler de Lambda que se declara `text/html`. `DOMPurify.sanitize`,
`sanitize-html`, `escape-html`, `$sanitize` o una función propia que se llame "escape…html"
limpian el HTML; `path.basename`, la ruta. Cada función (también las anónimas de una ruta) es un
método del motor, y entre archivos se resuelven `import` y `require`, también los relativos.

## Go

En Go, las mismas cuatro reglas, y como Go declara los tipos, las fuentes y los sumideros salen de
ellos: un parámetro `*http.Request` o un evento de Lambda (`events.APIGatewayProxyRequest`) trae
todo del usuario, y de un `*gin.Context` o un `echo.Context` sólo lo que lee el pedido (`Query`,
`Param`, `PostForm`...). Los sumideros: `Query`, `QueryRow`, `Exec` y `Prepare` (y sus
`...Context`, con la consulta después del `ctx`) de `*sql.DB`, `sqlx` y `pgx`, y `Raw`, `Where`,
`Order` y compañía de gorm, siguiendo el tipo por la cadena (`db.Table("x").Order(campo)`);
`exec.Command` con una shell y `-c`; `os.Open`, `os.ReadFile`, `os.Remove` y compañía y
`http.ServeFile`; y `w.Write`, `fmt.Fprintf(w, ...)`, `io.WriteString(w, ...)` sobre un
`http.ResponseWriter` (no si la función pone un `Content-Type` que no es HTML), `template.HTML` y
la respuesta de Lambda que se declara `text/html`. `html.EscapeString`, `template.HTMLEscapeString`
y `json.Marshal` (escapa `<` y `>`) limpian el HTML; `filepath.Base` y `path.Clean("/" + ruta)`, la
ruta; `strconv.Atoi` y compañía dan un número. Lo escrito en un `strings.Builder` queda en él. Las
funciones anónimas (el `func(w, r)` de un `HandleFunc`) ven los tipos de las variables de las que
las contienen, y entre paquetes las llamadas se resuelven por la carpeta: la ruta del import
termina como la carpeta del paquete.

## Secretos y dependencias vulnerables

`committed-secret` revisa todos los archivos versionados, no sólo el código, con las 222 reglas de
la configuración por defecto de gitleaks (claves de AWS, GCP y Azure, tokens de GitHub, GitLab,
Slack y Stripe, claves privadas, JWT…), sus allowlists, su umbral de entropía y su decodificación
de base64, hex, URL y escapes Unicode, más una regla propia para contraseñas en cadenas de
conexión. Marca lo mismo que `gitleaks dir` en los repos donde se midió. El secreto nunca sale
entero en el reporte ni se guarda en la caché: sólo los primeros cuatro caracteres y el largo. Un
`gitleaks:allow` en la línea lo deja pasar; los datos de prueba se excluyen en `kerbo.toml` con
`[[exceptions]]`, `rule = "committed-secret"` y su `reason` (los `.gitleaksignore` no se leen,
para que nada se afloje en silencio).

`vulnerable-dependency` es la única regla que usa la red, y sólo con `--osv`: lee `Cargo.lock`,
`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `go.mod`, `go.sum`, `poetry.lock`, `uv.lock`,
`Pipfile.lock`, `requirements*.txt` fijados con `==`, `pom.xml` (con los padres locales y sus
propiedades) y `gradle.lockfile`, y le pregunta a OSV por las versiones exactas. Habla con
`api.osv.dev` a través de `curl` (el que trae el sistema), con un minuto de tope por pedido, y
guarda las respuestas en la caché un día; los avisos se guardan hasta que OSV los modifique.
`kerbo osv-download` baja la base de cada ecosistema (unos 280 MB en total, npm es casi todo) y
`--osv=offline` la usa sin red. Cada problema dice el aviso, sus alias y la versión que lo arregla.
Las dos reglas están en los tres presets, también en los tests.
