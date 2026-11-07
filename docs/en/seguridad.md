# Security

## Unsanitized user data

And for security, what reaches a query (`sql-injection`), a command (`command-injection`), a file
path (`path-traversal`) or the HTML response (`xss`) from the user unsanitized. Sources are the `HttpServletRequest`
methods that return what the user sent (`getParameter`, `getHeader`, `getCookies`,
`getQueryString`...) and parameters with `@RequestParam`, `@PathVariable`, `@RequestBody` and the
other Spring web annotations. Sinks are `Statement`, `Connection`, `JdbcTemplate`,
`EntityManager` and `JdbcClient` (in a query only the text counts: `?` arguments are safe),
`Runtime.exec` and `ProcessBuilder`, and `File`, `FileInputStream`, `FileOutputStream`,
`FileReader`, `FileWriter`, `RandomAccessFile`, `Paths.get` and `Path.of`, and what is written
through `response.getWriter()` or `getOutputStream()` (chained or kept in a variable). An HTML
escape (ESAPI's `encodeForHTML`, `HtmlUtils.htmlEscape`, `escapeHtml4`, `Encode.forHtml`...)
protects the response but not a query or a path: escaped data is followed separately. All of it is
a data table in `src/lint/java_taint/tables.rs`.

Each method is walked like an interpreter: it knows which variables are constants and uses them to
decide `if`, `switch` and ternaries (`(7 * 42) - 86 > 200 ? "fixed" : data` always takes the same
branch), and it follows the lists it builds position by position and its maps key by key (taking
`list.get(1)` when the data went into slot 0 is not tainted). Across methods and files, each one
leaves a summary (what it returns depending on its parameters and what reaches each sink) and the
index combines them: a call goes to the methods of its type, to the ones it inherits and to its
implementations; a type in the same file shadows same-named ones elsewhere. What an abstract
method of an interface nobody implements in the code returns (a Spring Data repository) comes from
the database, not from its arguments. A library call returns what its receiver and arguments
bring, except the ones that give a number, a boolean or a UUID (`parseInt`, `length`, `equals`,
`UUID.fromString`...).

## Python

In Python, the same four rules with the same engine. Sources are Flask's `request.args`, `.form`,
`.cookies`, `.headers`, `.values`, `.files`, `.json`, `.get_json()` and friends, Django's (`GET`,
`POST`, `COOKIES`, `META`, `body`) and Pyramid's (`params`, `matchdict`), the parameters of a route
function (`@app.route`, `@router.get`...) and the `event` of a Lambda handler. Sinks: `.execute`,
`.executemany` and `.executescript`, `text()` and the SQLAlchemy `Query` methods that take SQL as
text; `os.system`, `os.popen` and `os.exec*`/`spawn*` always, and `subprocess.*` only with
`shell=True` or when what it runs is a shell with `-c` (without a shell, a built string is not
interpreted); `open`, `codecs.open`, `os.remove`, `os.path.exists`, `shutil.*`, `send_file` and the
methods of a `pathlib` object; and the response body: what a Flask view returns (not when it
returns a dict or uses `jsonify`; in FastAPI it is JSON), `make_response` and
`render_template_string`. An `if '..' in path: return` validates the path; an HTML escape
(`html.escape`, `markupsafe.escape`, `render_template` or a project function named "escape…html"),
`shlex.quote` and `secure_filename` clean for their sink; what builds a SQLAlchemy statement
(`select`, `insert(...).values(...)`) uses bound parameters. Across modules, calls are resolved
through imports (relative ones and the ones inside a function too), the functions and classes
visible from each one, `self` and the class a variable was created with. `os.name` is taken as
`"posix"`: that is where the server runs.

## JavaScript and TypeScript

In JavaScript and TypeScript, the same four rules. Without types, sources and sinks are recognized
by name: the request (`req`, `request`, `ctx`) and its `query`, `params`, `body`, `cookies`,
`headers`, `url` and friends; the `event` of a Lambda handler (`(event, context)`, or a lone
`event` in a function called `handler`); and in the browser, `location.hash`, `location.search`,
`document.URL`, `document.referrer` and `window.name`. Sinks: `query`, `execute`, `raw` and
`$queryRawUnsafe` on a connection (`db`, `conn`, `pool`, `client`, `knex`, `prisma`...);
`child_process.exec` always and `spawn`/`execFile` with `{ shell: true }` or with a program chosen by the user; `fs.*` and
`res.sendFile`; and for HTML, `res.send`, `res.write` and `res.end` from Express and Node (not
Fastify's, which sends plain text, nor when the function declares another type with
`res.type('json')` or `writeHead(200, { 'content-type': 'application/json' })`), `innerHTML`,
`document.write`, `insertAdjacentHTML`, `dangerouslySetInnerHTML`, jQuery (`$("<b>" + x)`,
`.html(x)`) and the response of a Lambda handler declared as `text/html`. `DOMPurify.sanitize`,
`sanitize-html`, `escape-html`, `$sanitize` or a project function named "escape…html" clean HTML;
`path.basename`, the path. Every function (anonymous route handlers too) is a method for the
engine, and across files `import` and `require` are resolved, relative ones too.

## Go

In Go, the same four rules, and since Go declares types, sources and sinks come from them: a
`*http.Request` parameter or a Lambda event (`events.APIGatewayProxyRequest`) brings everything
from the user, and from a `*gin.Context` or an `echo.Context` only what reads the request
(`Query`, `Param`, `PostForm`...). Sinks: `Query`, `QueryRow`, `Exec` and `Prepare` (and their
`...Context` variants, with the query after `ctx`) on `*sql.DB`, `sqlx` and `pgx`, and gorm's
`Raw`, `Where`, `Order` and friends, following the type along the chain
(`db.Table("x").Order(field)`); `exec.Command` with a shell and `-c`; `os.Open`, `os.ReadFile`,
`os.Remove` and friends and `http.ServeFile`; and `w.Write`, `fmt.Fprintf(w, ...)`,
`io.WriteString(w, ...)` on an `http.ResponseWriter` (not when the function sets a `Content-Type`
that is not HTML), `template.HTML` and a Lambda response declared as `text/html`.
`html.EscapeString`, `template.HTMLEscapeString` and `json.Marshal` (it escapes `<` and `>`) clean
HTML; `filepath.Base` and `path.Clean("/" + path)`, the path; `strconv.Atoi` and friends give a
number. What is written into a `strings.Builder` stays in it. Anonymous functions (the `func(w, r)`
of a `HandleFunc`) see the types of the variables of the functions that contain them, and across
packages calls are resolved by directory: the import path ends like the package's directory.

## Secrets and vulnerable dependencies

`committed-secret` checks every committed file, not just code, with the 222 rules of gitleaks'
default configuration (AWS, GCP and Azure keys, GitHub, GitLab, Slack and Stripe tokens, private
keys, JWTs…), its allowlists, its entropy threshold and its decoding of base64, hex, URL and
Unicode escapes, plus a rule of its own for passwords in connection strings. It reports the same
as `gitleaks dir` on the repos where it was measured. The secret is never printed whole nor stored
in the cache: only its first four characters and its length. A `gitleaks:allow` on the line lets
it through; test data is excluded in `kerbo.toml` with `[[exceptions]]`,
`rule = "committed-secret"` and its `reason` (`.gitleaksignore` files are not read, so nothing is
loosened silently).

`vulnerable-dependency` is the only rule that uses the network, and only with `--osv`: it reads
`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `go.mod`, `go.sum`,
`poetry.lock`, `uv.lock`, `Pipfile.lock`, `requirements*.txt` pinned with `==`, `pom.xml` (with its
local parents and their properties) and `gradle.lockfile`, and asks OSV about the exact versions.
It talks to `api.osv.dev` through the system's `curl`, with a one-minute limit per request, and
keeps the answers in the cache for a day; advisories are kept until OSV modifies them.
`kerbo osv-download` downloads each ecosystem's database (about 280 MB in all, almost all of it
npm) and `--osv=offline` uses it without network. Each problem names the advisory, its aliases and
the version that fixes it. Both rules are in all three presets, tests included.
