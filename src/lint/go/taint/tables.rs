use crate::lint::taint::SinkKind;
use crate::lint::taint::responses::is_named_html_escape;
pub use crate::lint::taint::responses::{is_content_type_header, is_html_type};

// Los tipos que traen el pedido entero: lo que se lea de ellos lo manda el usuario.
const REQUEST_TYPES: &[&str] = &[
    "net/http.Request",
    "github.com/aws/aws-lambda-go/events.APIGatewayProxyRequest",
    "github.com/aws/aws-lambda-go/events.APIGatewayV2HTTPRequest",
    "github.com/aws/aws-lambda-go/events.ALBTargetGroupRequest",
    "github.com/aws/aws-lambda-go/events.LambdaFunctionURLRequest",
];
// Los contextos de gin, echo y fiber: sólo algunos de sus métodos leen el pedido.
const CONTEXT_TYPES: &[&str] = &[
    "github.com/gin-gonic/gin.Context",
    "github.com/labstack/echo.Context",
    "github.com/gofiber/fiber.Ctx",
];
const CONTEXT_SOURCES: &[&str] = &[
    "Query",
    "DefaultQuery",
    "QueryArray",
    "QueryMap",
    "Param",
    "Params",
    "PostForm",
    "DefaultPostForm",
    "PostFormArray",
    "GetHeader",
    "Cookie",
    "GetRawData",
    "FormValue",
    "QueryParam",
    "QueryParams",
    "FormParams",
    "Body",
];
const CONTEXT_REQUEST: &str = "Request";
const WRITER_TYPE: &str = "net/http.ResponseWriter";
const WRITER_METHODS: &[&str] = &["Write", "WriteString"];
// Lo que escribe en el `io.Writer` de su primer argumento.
const PRINTERS: &[&str] = &["fmt.Fprintf", "fmt.Fprint", "fmt.Fprintln", "io.WriteString"];
const HTML_CONVERSIONS: &[&str] = &["html/template.HTML", "html/template.JS", "html/template.HTMLAttr"];
// `text/template` no escapa: lo que se le pasa a `Execute` sobre la respuesta sale como HTML.
const TEXT_TEMPLATE: &str = "text/template.Template";
const TEXT_TEMPLATE_BUILDERS: &[&str] = &[
    "text/template.New",
    "text/template.Must",
    "text/template.ParseFiles",
    "text/template.ParseGlob",
    "text/template.ParseFS",
];
const TEXT_TEMPLATE_CHAIN: &[&str] = &[
    "New",
    "Parse",
    "ParseFiles",
    "ParseGlob",
    "ParseFS",
    "Funcs",
    "Option",
    "Lookup",
];
const TEXT_TEMPLATE_EXECUTE: &[&str] = &["Execute", "ExecuteTemplate"];
const SQL_TYPES: &[&str] = &[
    "database/sql.DB",
    "database/sql.Tx",
    "database/sql.Conn",
    "github.com/jmoiron/sqlx.DB",
    "github.com/jmoiron/sqlx.Tx",
    "github.com/jackc/pgx.Conn",
    "github.com/jackc/pgx/pgxpool.Pool",
];
const SQL_METHODS: &[&str] = &["Query", "QueryRow", "Exec", "Prepare", "Queryx", "QueryRowx"];
const CONTEXT_SUFFIX: &str = "Context";
// Sin tipo conocido, el nombre es lo que hay: como en JavaScript.
const SQL_RECEIVERS: &[&str] = &["db", "tx", "conn", "database", "pool"];
const GORM_TYPES: &[&str] = &["gorm.io/gorm.DB", "github.com/jinzhu/gorm.DB"];
const GORM_METHODS: &[&str] = &[
    "Raw", "Exec", "Where", "Or", "Not", "Order", "Select", "Group", "Having", "Joins", "Distinct",
];
// Lo que abre una conexión, y de qué tipo es lo que devuelve.
const OPENERS: &[(&str, &str)] = &[
    ("database/sql.Open", "database/sql.DB"),
    ("gorm.io/gorm.Open", "gorm.io/gorm.DB"),
    ("github.com/jinzhu/gorm.Open", "github.com/jinzhu/gorm.DB"),
    ("github.com/jmoiron/sqlx.Open", "github.com/jmoiron/sqlx.DB"),
    ("github.com/jmoiron/sqlx.Connect", "github.com/jmoiron/sqlx.DB"),
    ("github.com/jackc/pgx.Connect", "github.com/jackc/pgx.Conn"),
    (
        "github.com/jackc/pgx/pgxpool.New",
        "github.com/jackc/pgx/pgxpool.Pool",
    ),
    (
        "github.com/jackc/pgx/pgxpool.Connect",
        "github.com/jackc/pgx/pgxpool.Pool",
    ),
];
const TRANSACTION_METHODS: &[&str] = &["Begin", "BeginTx"];
const TRANSACTION_TYPE: &str = "database/sql.Tx";
const COMMANDS: &[&str] = &["os/exec.Command", "os/exec.CommandContext"];
const COMMAND_WITH_CONTEXT: &str = "os/exec.CommandContext";
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "/bin/sh",
    "/bin/bash",
    "cmd",
    "cmd.exe",
    "powershell",
];
const SHELL_FLAGS: &[&str] = &["-c", "/c", "-Command"];
// Las funciones que abren, leen o borran la ruta que reciben, y en qué posición la reciben.
const PATH_FUNCTIONS: &[(&str, usize)] = &[
    ("os.Open", 0),
    ("os.OpenFile", 0),
    ("os.Create", 0),
    ("os.ReadFile", 0),
    ("os.WriteFile", 0),
    ("os.Remove", 0),
    ("os.RemoveAll", 0),
    ("os.Mkdir", 0),
    ("os.MkdirAll", 0),
    ("os.ReadDir", 0),
    ("os.Stat", 0),
    ("os.Lstat", 0),
    ("io/ioutil.ReadFile", 0),
    ("io/ioutil.WriteFile", 0),
    ("io/ioutil.ReadDir", 0),
    ("net/http.ServeFile", 2),
];
// `c.Data(200, "text/html", cuerpo)` de gin: el tipo va en el segundo argumento.
const CONTEXT_DATA: &str = "Data";
const CONTEXT_FILE_METHODS: &[&str] = &["File", "FileAttachment", "SendFile", "Attachment"];
const HTML_ESCAPES: &[&str] = &[
    "html.EscapeString",
    "html/template.HTMLEscapeString",
    "html/template.HTMLEscaper",
    "text/template.HTMLEscapeString",
    "net/url.QueryEscape",
    "encoding/json.Marshal",
    "encoding/json.MarshalIndent",
];
const PATH_ESCAPES: &[&str] = &["path/filepath.Base", "path.Base"];
// `path.Clean("/" + x)`: con la raíz adelante, Clean no deja ningún `..`.
pub const ROOTED_CLEANS: &[&str] = &["path.Clean", "path/filepath.Clean"];
pub const ROOT: &str = "/";
const CLEAN_RESULTS: &[&str] = &[
    "len",
    "cap",
    "Atoi",
    "ParseInt",
    "ParseUint",
    "ParseFloat",
    "ParseBool",
    "Contains",
    "HasPrefix",
    "HasSuffix",
    "EqualFold",
    "Index",
    "Count",
    "MatchString",
    "Match",
    "Len",
];
// Lo que se escribe en un `strings.Builder` o un `bytes.Buffer` queda en él.
const BUILDER_WRITES: &[&str] = &["WriteString", "WriteByte", "WriteRune"];
const HEADER_SETTERS: &[&str] = &["Set", "Add"];
pub const HEADER_CALL: &str = "Header()";

pub fn is_request_type(written: &str) -> bool {
    REQUEST_TYPES.contains(&written)
}

pub fn is_context_type(written: &str) -> bool {
    CONTEXT_TYPES.contains(&written)
}

pub fn is_context_source(method: &str) -> bool {
    CONTEXT_SOURCES.contains(&method)
}

pub fn is_context_request(field: &str) -> bool {
    field == CONTEXT_REQUEST
}

pub fn is_writer_type(written: &str) -> bool {
    written == WRITER_TYPE
}

pub fn is_writer_method(method: &str) -> bool {
    WRITER_METHODS.contains(&method)
}

pub fn is_text_template_execute(receiver_type: Option<&str>, method: &str) -> bool {
    receiver_type == Some(TEXT_TEMPLATE) && TEXT_TEMPLATE_EXECUTE.contains(&method)
}

pub fn is_printer(qualified: &str) -> bool {
    PRINTERS.contains(&qualified)
}

pub fn is_html_conversion(qualified: &str) -> bool {
    HTML_CONVERSIONS.contains(&qualified)
}

// Si el método recibe la consulta, en qué posición: después del `ctx` en los `...Context`.
pub fn sql_query_position(receiver_type: Option<&str>, receiver_name: &str, method: &str) -> Option<usize> {
    let base = method.strip_suffix(CONTEXT_SUFFIX).unwrap_or(method);
    let with_context = usize::from(base.len() < method.len());
    let gorm = receiver_type.is_some_and(|written| GORM_TYPES.contains(&written));
    if gorm {
        return GORM_METHODS.contains(&method).then_some(0);
    }
    let known = receiver_type.map_or_else(
        || SQL_RECEIVERS.contains(&receiver_name),
        |written| SQL_TYPES.contains(&written),
    );
    (known && SQL_METHODS.contains(&base)).then_some(with_context)
}

// El tipo de lo que devuelve una llamada: una conexión, una transacción o la misma cadena de gorm.
pub fn returned_type(qualified: &str, receiver_type: Option<&str>, method: &str) -> Option<String> {
    if let Some((_, opened)) = OPENERS.iter().find(|(opener, _)| *opener == qualified) {
        return Some((*opened).to_owned());
    }
    if TEXT_TEMPLATE_BUILDERS.contains(&qualified) {
        return Some(TEXT_TEMPLATE.to_owned());
    }
    let receiver = receiver_type?;
    if receiver == TEXT_TEMPLATE && TEXT_TEMPLATE_CHAIN.contains(&method) {
        return Some(TEXT_TEMPLATE.to_owned());
    }
    if GORM_TYPES.contains(&receiver) {
        return Some(receiver.to_owned());
    }
    (SQL_TYPES.contains(&receiver) && TRANSACTION_METHODS.contains(&method))
        .then(|| TRANSACTION_TYPE.to_owned())
}

pub fn is_command(qualified: &str) -> bool {
    COMMANDS.contains(&qualified)
}

pub fn command_offset(qualified: &str) -> usize {
    usize::from(qualified == COMMAND_WITH_CONTEXT)
}

pub fn is_shell(program: &str) -> bool {
    SHELLS.contains(&program)
}

pub fn is_shell_flag(flag: &str) -> bool {
    SHELL_FLAGS.contains(&flag)
}

pub fn path_position(qualified: &str) -> Option<usize> {
    PATH_FUNCTIONS
        .iter()
        .find(|(function, _)| *function == qualified)
        .map(|(_, position)| *position)
}

pub fn is_context_data(method: &str) -> bool {
    method == CONTEXT_DATA
}

pub fn is_context_file_method(method: &str) -> bool {
    CONTEXT_FILE_METHODS.contains(&method)
}

pub fn escapes(qualified: &str) -> Option<SinkKind> {
    let simple = qualified.rsplit('.').next().unwrap_or(qualified).to_lowercase();
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

pub fn is_builder_write(method: &str) -> bool {
    BUILDER_WRITES.contains(&method)
}

pub fn is_header_setter(method: &str) -> bool {
    HEADER_SETTERS.contains(&method)
}
