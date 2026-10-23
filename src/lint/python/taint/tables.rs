use crate::lint::taint::SinkKind;
use crate::lint::taint::responses::is_named_html_escape;

// Los objetos que traen lo que manda el usuario (el `request` de Flask, Django o Starlette, también
// como atributo: `self.request`) y lo que se lee de ellos.
const REQUEST_OBJECTS: &[&str] = &["request", "req", "self.request", "flask.request"];
const REQUEST_ATTRIBUTES: &[&str] = &[
    "args",
    "form",
    "values",
    "cookies",
    "headers",
    "files",
    "json",
    "data",
    "query_string",
    "path",
    "full_path",
    "url",
    "base_url",
    "environ",
    "stream",
    "path_params",
    "query_params",
    "GET",
    "POST",
    "COOKIES",
    "META",
    "FILES",
    "body",
    "params",
    "matchdict",
    "json_body",
];
const REQUEST_METHODS: &[&str] = &["get_json", "get_data", "json", "body", "form"];
// Los decoradores de una función que atiende una ruta: lo que devuelve es la respuesta, y sus
// parámetros vienen de la URL o del pedido.
const ROUTE_DECORATORS: &[&str] = &[
    ".route",
    ".get",
    ".post",
    ".put",
    ".patch",
    ".delete",
    ".api_route",
];
// Lo que devuelven no puede llevar texto armado por el usuario.
const CLEAN_RESULTS: &[&str] = &[
    "int",
    "float",
    "bool",
    "len",
    "abs",
    "round",
    "ord",
    "hash",
    "id",
    "isinstance",
    "callable",
    "isdigit",
    "isalnum",
    "isalpha",
    "isnumeric",
    "isdecimal",
    "startswith",
    "endswith",
    "count",
    "find",
    "index",
    "rfind",
    "exists",
    "isfile",
    "isdir",
    "uuid4",
    "UUID",
    "bindparams",
];
const SQL_METHODS: &[&str] = &[
    "execute",
    "executemany",
    "executescript",
    "raw",
    "read_sql",
    "read_sql_query",
];
const SQL_FUNCTIONS: &[&str] = &[
    "sqlalchemy.text",
    "sqlalchemy.sql.text",
    "pandas.read_sql",
    "pandas.read_sql_query",
];
const COMMAND_FUNCTIONS: &[&str] = &[
    "os.system",
    "os.popen",
    "os.execl",
    "os.execle",
    "os.execlp",
    "os.execlpe",
    "os.execv",
    "os.execve",
    "os.execvp",
    "os.execvpe",
    "os.spawnl",
    "os.spawnle",
    "os.spawnlp",
    "os.spawnlpe",
    "os.spawnv",
    "os.spawnve",
    "os.spawnvp",
    "os.spawnvpe",
    "os.startfile",
    "subprocess.getoutput",
    "subprocess.getstatusoutput",
    "asyncio.create_subprocess_shell",
    "asyncio.subprocess.create_subprocess_shell",
];
// Los que corren un programa sin shell: sólo son inyección con `shell=True` o si lo que corren es una
// shell con `-c` (`["sh", "-c", f"echo {x}"]`).
const PROGRAM_FUNCTIONS: &[&str] = &[
    "subprocess.run",
    "subprocess.call",
    "subprocess.check_call",
    "subprocess.check_output",
    "subprocess.Popen",
    "asyncio.create_subprocess_exec",
    "asyncio.subprocess.create_subprocess_exec",
];
// Los métodos de un `Query` de SQLAlchemy que reciben SQL en texto.
const QUERY_METHODS: &[&str] = &[
    "distinct",
    "having",
    "group_by",
    "order_by",
    "filter",
    "from_statement",
    "params",
];
const QUERY_FACTORY: &str = "query";
pub const QUERY_OBJECT: &str = "sqlalchemy.orm.Query";

// Lo que kerbo da por sabido de la máquina donde corre el servidor: POSIX. Así se pliegan los
// `if "Windows" in os.name` que eligen la shell.
const PLATFORM_CONSTANTS: &[(&str, &str)] = &[("os.name", "posix"), ("sys.platform", "linux")];

pub fn platform_constant(qualified: &str) -> Option<&'static str> {
    PLATFORM_CONSTANTS
        .iter()
        .find(|(name, _)| *name == qualified)
        .map(|(_, value)| *value)
}

pub fn runs_program(qualified: &str) -> bool {
    PROGRAM_FUNCTIONS.contains(&qualified)
}

pub fn creates_query(method: &str) -> bool {
    method == QUERY_FACTORY
}

pub fn query_method_sink(method: &str) -> Option<SinkKind> {
    QUERY_METHODS.contains(&method).then_some(SinkKind::Sql)
}
const PATH_FUNCTIONS: &[&str] = &[
    "builtins.open",
    "codecs.open",
    "io.open",
    "os.open",
    "os.remove",
    "os.unlink",
    "os.rmdir",
    "os.removedirs",
    "os.mkdir",
    "os.makedirs",
    "os.listdir",
    "os.scandir",
    "os.rename",
    "os.replace",
    "os.chmod",
    "shutil.copy",
    "shutil.copy2",
    "shutil.copyfile",
    "shutil.copytree",
    "shutil.move",
    "shutil.rmtree",
    "pathlib.Path",
    "pathlib.PurePath",
    "os.path.exists",
    "os.path.isfile",
    "os.path.isdir",
    "os.path.getsize",
    "os.path.getmtime",
    "flask.send_file",
    "fastapi.responses.FileResponse",
];
const HTML_FUNCTIONS: &[&str] = &[
    "flask.make_response",
    "flask.Response",
    "flask.render_template_string",
    "django.http.HttpResponse",
    "markupsafe.Markup",
    "flask.Markup",
    "jinja2.Markup",
    "django.utils.safestring.mark_safe",
    "django.utils.safestring.SafeString",
    "jinja2.Template",
    "starlette.responses.HTMLResponse",
    "fastapi.responses.HTMLResponse",
];
// Los escapes que dejan el texto seguro para un sumidero en particular.
const HTML_ESCAPES: &[&str] = &[
    "html.escape",
    "markupsafe.escape",
    "flask.escape",
    "cgi.escape",
    "bleach.clean",
    "django.utils.html.escape",
    "django.utils.html.conditional_escape",
    "flask.render_template",
    "flask.jsonify",
    "django.shortcuts.render",
    "django.template.loader.render_to_string",
];
const COMMAND_ESCAPES: &[&str] = &["shlex.quote", "pipes.quote"];
const PATH_ESCAPES: &[&str] = &["werkzeug.utils.secure_filename", "os.path.basename"];
const LAMBDA_PARAMETERS: [&str; 2] = ["event", "context"];
const QUOTES: &[char] = &['\'', '"'];
const VARIABLE_PART: char = '<';
const RESPONSES: &[&str] = &[
    "flask.make_response",
    "flask.Response",
    "django.http.HttpResponse",
];

// Las respuestas cuyo primer argumento es el cuerpo: lo demás son el estado y los headers.
pub fn is_response(qualified: &str) -> bool {
    RESPONSES.contains(&qualified)
}

pub fn is_request_object(written: &str) -> bool {
    REQUEST_OBJECTS.contains(&written)
}

pub fn is_request_attribute(name: &str) -> bool {
    REQUEST_ATTRIBUTES.contains(&name)
}

pub fn is_request_method(name: &str) -> bool {
    REQUEST_METHODS.contains(&name)
}

pub fn is_route_decorator(written: &str) -> bool {
    let callee = written.split('(').next().unwrap_or(written);
    ROUTE_DECORATORS.iter().any(|suffix| callee.ends_with(suffix))
}

// La ruta de un decorador de Flask sin partes variables (`@app.route('/a/b')`): ahí `request.path`
// es siempre ese texto.
pub fn fixed_route(written: &str) -> Option<String> {
    if !is_route_decorator(written) {
        return None;
    }
    let opened = written.find(QUOTES)?;
    let quote = written.get(opened..)?.chars().next()?;
    let rest = written.get(opened.saturating_add(1)..)?;
    let route = rest.get(..rest.find(quote)?)?;
    (!route.contains(VARIABLE_PART)).then(|| route.to_owned())
}

pub fn is_lambda_handler(parameters: &[String]) -> bool {
    parameters.len() == LAMBDA_PARAMETERS.len()
        && parameters
            .iter()
            .zip(LAMBDA_PARAMETERS)
            .all(|(written, expected)| written == expected)
}

pub fn gives_clean_result(name: &str) -> bool {
    CLEAN_RESULTS.contains(&name)
}

// El tipo de sumidero de una función por su nombre completo, o de un método por su nombre.
pub fn function_sink(qualified: &str) -> Option<SinkKind> {
    if SQL_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Sql)
    } else if COMMAND_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Command)
    } else if PATH_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Path)
    } else if HTML_FUNCTIONS.contains(&qualified) {
        Some(SinkKind::Html)
    } else {
        None
    }
}

// Lo que arma una sentencia de SQLAlchemy (`select`, `insert(...).values(...)`): los datos van como
// parámetros ligados, así que no son inyección. `text` es la excepción.
const STATEMENT_LIBRARY: &str = "sqlalchemy.";

pub fn is_statement_builder(qualified: &str) -> bool {
    qualified.starts_with(STATEMENT_LIBRARY) && !SQL_FUNCTIONS.contains(&qualified)
}

// Un sumidero que arma la consulta: lo que devuelve ya se reportó donde se armó.
pub fn builds_query(qualified: &str) -> bool {
    SQL_FUNCTIONS.contains(&qualified)
}

pub fn method_sink(name: &str) -> Option<SinkKind> {
    SQL_METHODS.contains(&name).then_some(SinkKind::Sql)
}

// Los métodos de un objeto de `pathlib` que tocan el archivo.
const PATH_METHODS: &[&str] = &[
    "exists",
    "is_file",
    "is_dir",
    "open",
    "read_text",
    "read_bytes",
    "write_text",
    "write_bytes",
    "unlink",
    "mkdir",
    "rmdir",
    "touch",
    "iterdir",
    "stat",
    "rename",
    "replace",
    "glob",
    "rglob",
];
const PATH_MODULE: &str = "pathlib.";
const CONFIG_PARSERS: &[&str] = &[
    "configparser.ConfigParser",
    "configparser.RawConfigParser",
    "configparser.SafeConfigParser",
];
const EMPTY_LISTS: &[&str] = &["builtins.list"];
const EMPTY_MAPS: &[&str] = &["builtins.dict"];

pub fn is_path_object(created: &str) -> bool {
    created.starts_with(PATH_MODULE)
}

pub fn path_method_sink(name: &str) -> Option<SinkKind> {
    PATH_METHODS.contains(&name).then_some(SinkKind::Path)
}

pub fn is_config_parser(created: &str) -> bool {
    CONFIG_PARSERS.contains(&created)
}

pub fn creates_list(qualified: &str) -> bool {
    EMPTY_LISTS.contains(&qualified)
}

pub fn creates_map(qualified: &str) -> bool {
    EMPTY_MAPS.contains(&qualified) || CONFIG_PARSERS.contains(&qualified)
}

// Para qué sumideros deja de ser peligroso lo que devuelve esta función.
pub fn escapes(qualified: &str) -> Option<SinkKind> {
    let simple = qualified.rsplit('.').next().unwrap_or(qualified).to_lowercase();
    if HTML_ESCAPES.contains(&qualified) || is_named_html_escape(&simple) {
        Some(SinkKind::Html)
    } else if COMMAND_ESCAPES.contains(&qualified) {
        Some(SinkKind::Command)
    } else if PATH_ESCAPES.contains(&qualified) {
        Some(SinkKind::Path)
    } else {
        None
    }
}

pub use crate::lint::taint::commands::{is_shell, is_shell_command_flag};
