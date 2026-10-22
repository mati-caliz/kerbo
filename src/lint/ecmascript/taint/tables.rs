use crate::lint::taint::SinkKind;
use crate::lint::taint::responses::is_named_html_escape;
pub use crate::lint::taint::responses::{is_content_type_header, is_html_type};

// Los objetos de un pedido (Express, Koa, Next, Fastify, Lambda) y lo que traen del usuario.
const REQUEST_OBJECTS: &[&str] = &["req", "request", "ctx", "ctx.request", "event"];
const REQUEST_PROPERTIES: &[&str] = &[
    "query",
    "params",
    "body",
    "cookies",
    "signedCookies",
    "headers",
    "url",
    "originalUrl",
    "path",
    "hostname",
    "files",
    "file",
    "queryStringParameters",
    "pathParameters",
    "multiValueQueryStringParameters",
    "nextUrl",
];
const REQUEST_METHODS: &[&str] = &["get", "header", "param", "json", "text", "formData"];
// El pedido de Hono: `c.req.query('x')`, `c.req.param('id')`, `await c.req.json()`.
const HONO_REQUEST: &str = "c.req";
const HONO_METHODS: &[&str] = &[
    "query",
    "queries",
    "param",
    "header",
    "json",
    "text",
    "formData",
    "parseBody",
];
// Los decoradores de Nest que llenan un parámetro con lo que manda el usuario.
const NEST_SOURCES: &[&str] = &[
    "Query",
    "Param",
    "Body",
    "Headers",
    "Req",
    "Request",
    "UploadedFile",
    "UploadedFiles",
];
// Las props de una página de Next: lo que viene en la URL.
const NEXT_PROPS: &[&str] = &["searchParams"];
const DECORATOR_MARK: char = '@';
const ARGUMENTS_START: char = '(';
// Lo que el navegador trae de la URL o de otra ventana.
const BROWSER_SOURCES: &[&str] = &[
    "location.hash",
    "location.search",
    "location.href",
    "location.pathname",
    "window.location.hash",
    "window.location.search",
    "window.location.href",
    "document.location.hash",
    "document.location.href",
    "document.URL",
    "document.documentURI",
    "document.referrer",
    "window.name",
];
const SQL_METHODS: &[&str] = &[
    "query",
    "execute",
    "raw",
    "$queryRawUnsafe",
    "$executeRawUnsafe",
    "run",
    "all",
    "exec",
];
// Los receptores que suelen ser una conexión: sin tipos, el nombre es lo que hay.
const SQL_RECEIVERS: &[&str] = &[
    "db",
    "conn",
    "connection",
    "pool",
    "client",
    "sequelize",
    "knex",
    "prisma",
    "database",
    "sql",
    "mysql",
    "pg",
    "request",
];
const COMMAND_FUNCTIONS: &[&str] = &[
    "child_process.exec",
    "child_process.execSync",
    "child_process.spawn",
    "child_process.spawnSync",
    "child_process.execFile",
    "child_process.execFileSync",
];
const SHELL_COMMANDS: &[&str] = &["child_process.exec", "child_process.execSync"];
const PATH_FUNCTIONS: &[&str] = &[
    "fs.readFile",
    "fs.readFileSync",
    "fs.writeFile",
    "fs.writeFileSync",
    "fs.appendFile",
    "fs.appendFileSync",
    "fs.createReadStream",
    "fs.createWriteStream",
    "fs.unlink",
    "fs.unlinkSync",
    "fs.rm",
    "fs.rmSync",
    "fs.readdir",
    "fs.readdirSync",
    "fs.stat",
    "fs.statSync",
    "fs.existsSync",
    "fs.promises.readFile",
    "fs.promises.writeFile",
    "fs/promises.readFile",
    "fs/promises.writeFile",
];
const RESPONSE_OBJECTS: &[&str] = &["res", "response", "reply"];
// Fastify (`reply`) manda los textos como `text/plain`: sólo Express y Node escriben HTML.
const HTML_RESPONSE_OBJECTS: &[&str] = &["res", "response"];
const RESPONSE_HTML_METHODS: &[&str] = &["send", "write", "end"];
const RESPONSE_PATH_METHODS: &[&str] = &["sendFile", "download"];
const HTML_PROPERTIES: &[&str] = &["innerHTML", "outerHTML"];
const HTML_METHODS: &[&str] = &["insertAdjacentHTML", "bypassSecurityTrustHtml", "html"];
// jQuery arma elementos con el HTML que recibe: `$("<span>" + nombre + "</span>")`.
const HTML_FUNCTIONS: &[&str] = &["jQuery", "$"];
const DOCUMENT: &str = "document";
const DOCUMENT_METHODS: &[&str] = &["write", "writeln"];
// Lo que declara el tipo de una respuesta: si no es HTML, escribir en ella no es un XSS.
const CONTENT_TYPE_METHODS: &[&str] = &["set", "setHeader", "header", "type", "contentType", "writeHead"];
const LAMBDA_PARAMETERS: [&str; 2] = ["event", "context"];
const LAMBDA_EVENT: &str = "event";
const LAMBDA_HANDLER: &str = "handler";
const HTML_ESCAPES: &[&str] = &[
    "escape-html",
    "DOMPurify.sanitize",
    "sanitizeHtml",
    "sanitize-html",
    "he.encode",
    "he.escape",
    "validator.escape",
    "xss",
    "sanitizeUrl",
    "$sanitize",
    "@braintree/sanitize-url.sanitizeUrl",
];
const CLEAN_RESULTS: &[&str] = &[
    "parseInt",
    "parseFloat",
    "Number",
    "Boolean",
    "isNaN",
    "length",
    "includes",
    "startsWith",
    "endsWith",
    "indexOf",
    "test",
    "has",
    "existsSync",
];
const PATH_ESCAPES: &[&str] = &["path.basename"];
const NAME_BOUNDARIES: &[char] = &['.', '#'];

pub fn is_request_object(written: &str) -> bool {
    REQUEST_OBJECTS.contains(&written)
}

pub fn is_request_property(name: &str) -> bool {
    REQUEST_PROPERTIES.contains(&name)
}

pub fn is_request_method(name: &str) -> bool {
    REQUEST_METHODS.contains(&name)
}

pub fn is_hono_source(receiver: &str, method: &str) -> bool {
    receiver == HONO_REQUEST && HONO_METHODS.contains(&method)
}

// `@Query('id')` o `@Body()`: el nombre del decorador, sin la arroba ni los argumentos.
pub fn is_nest_source(decorator: &str) -> bool {
    let name = decorator.trim_start_matches(DECORATOR_MARK);
    let name = name.split(ARGUMENTS_START).next().unwrap_or(name).trim();
    NEST_SOURCES.contains(&name)
}

pub fn is_next_prop(name: &str) -> bool {
    NEXT_PROPS.contains(&name)
}

pub fn is_browser_source(written: &str) -> bool {
    BROWSER_SOURCES.contains(&written)
}

pub fn sql_method_sink(receiver: &str, method: &str) -> Option<SinkKind> {
    let last = receiver.rsplit('.').next().unwrap_or(receiver);
    (SQL_METHODS.contains(&method) && SQL_RECEIVERS.contains(&last)).then_some(SinkKind::Sql)
}

pub fn function_sink(qualified: &str) -> Option<SinkKind> {
    if COMMAND_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Command)
    } else if PATH_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Path)
    } else if HTML_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Html)
    } else {
        None
    }
}

// `exec` pasa por una shell; `spawn` y `execFile`, sólo con `{ shell: true }`.
pub fn always_shell(qualified: &str) -> bool {
    SHELL_COMMANDS.contains(&qualified)
}

pub fn response_sink(receiver: &str, method: &str) -> Option<SinkKind> {
    if !RESPONSE_OBJECTS.contains(&receiver) {
        return None;
    }
    if RESPONSE_HTML_METHODS.contains(&method) && HTML_RESPONSE_OBJECTS.contains(&receiver) {
        Some(SinkKind::Html)
    } else if RESPONSE_PATH_METHODS.contains(&method) {
        Some(SinkKind::Path)
    } else {
        None
    }
}

pub fn is_html_property(name: &str) -> bool {
    HTML_PROPERTIES.contains(&name)
}

pub fn html_method_sink(receiver: &str, method: &str) -> Option<SinkKind> {
    let on_document = receiver == DOCUMENT && DOCUMENT_METHODS.contains(&method);
    (on_document || HTML_METHODS.contains(&method)).then_some(SinkKind::Html)
}

pub fn declares_content_type(receiver: &str, method: &str) -> bool {
    RESPONSE_OBJECTS.contains(&receiver) && CONTENT_TYPE_METHODS.contains(&method)
}

// `(event, context)`, o un único `event` en una función que se llama `handler`.
pub fn is_lambda_handler(name: Option<&str>, parameters: &[&str]) -> bool {
    let named_handler = name.is_some_and(|written| written.rsplit('.').next() == Some(LAMBDA_HANDLER));
    let both = parameters.len() == LAMBDA_PARAMETERS.len()
        && parameters
            .iter()
            .zip(LAMBDA_PARAMETERS)
            .all(|(written, expected)| *written == expected);
    both || (named_handler && parameters.first() == Some(&LAMBDA_EVENT))
}

pub fn escapes(qualified: &str) -> Option<SinkKind> {
    let simple = qualified
        .rsplit(NAME_BOUNDARIES)
        .next()
        .unwrap_or(qualified)
        .to_lowercase();
    if HTML_ESCAPES.contains(&qualified) || is_named_html_escape(&simple) {
        Some(SinkKind::Html)
    } else if PATH_ESCAPES.contains(&qualified) {
        Some(SinkKind::Path)
    } else {
        None
    }
}

pub fn gives_clean_result(name: &str) -> bool {
    CLEAN_RESULTS.contains(&name)
}
