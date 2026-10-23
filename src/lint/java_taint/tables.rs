use crate::lint::taint::SinkKind;

// Los tipos que traen lo que manda el usuario y los métodos que lo devuelven.
const SOURCE_TYPES: &[&str] = &[
    "HttpServletRequest",
    "ServletRequest",
    "HttpServletRequestWrapper",
    "MultipartHttpServletRequest",
    "WebRequest",
    "NativeWebRequest",
];
const SOURCE_METHODS: &[&str] = &[
    "getParameter",
    "getParameterValues",
    "getParameterMap",
    "getParameterNames",
    "getHeader",
    "getHeaders",
    "getHeaderNames",
    "getCookies",
    "getQueryString",
    "getRequestURI",
    "getRequestURL",
    "getPathInfo",
    "getInputStream",
    "getReader",
    "getPart",
    "getParts",
];
// Los parámetros que Spring, JAX-RS (también Quarkus) y Micronaut llenan con lo que manda el usuario.
const SOURCE_PARAMETER_ANNOTATIONS: &[&str] = &[
    "RequestParam",
    "PathVariable",
    "RequestHeader",
    "CookieValue",
    "RequestBody",
    "RequestPart",
    "ModelAttribute",
    "MatrixVariable",
    "QueryParam",
    "PathParam",
    "FormParam",
    "HeaderParam",
    "CookieParam",
    "MatrixParam",
    "BeanParam",
    "RestQuery",
    "RestPath",
    "RestForm",
    "RestHeader",
    "RestCookie",
    "QueryValue",
    "Body",
    "Header",
    "Part",
];
// Lo que devuelven no puede llevar texto armado por el usuario: un número, un booleano, un UUID.
const CLEAN_RESULTS: &[&str] = &[
    "length",
    "size",
    "isEmpty",
    "isBlank",
    "equals",
    "equalsIgnoreCase",
    "hashCode",
    "contains",
    "containsKey",
    "containsValue",
    "startsWith",
    "endsWith",
    "indexOf",
    "lastIndexOf",
    "compareTo",
    "compareToIgnoreCase",
    "matches",
    "hasNext",
    "hasMoreElements",
    "exists",
    "isFile",
    "isDirectory",
    "parseInt",
    "parseLong",
    "parseShort",
    "parseByte",
    "parseDouble",
    "parseFloat",
    "parseBoolean",
];
const NUMERIC_TYPES: &[&str] = &[
    "Integer", "Long", "Short", "Byte", "Double", "Float", "Boolean", "UUID",
];
const NUMERIC_CONVERSIONS: &[&str] = &["valueOf", "fromString"];
// Los métodos que guardan sus argumentos adentro del objeto (`builder.append(x)`, `lista.add(x)`).
const ABSORBING_METHODS: &[&str] = &[
    "append",
    "insert",
    "add",
    "addAll",
    "put",
    "putAll",
    "set",
    "push",
    "offer",
    "addFirst",
    "addLast",
    "addElement",
];
// Los métodos cuya lambda recibe lo que trae el receptor: cada elemento de un stream o una
// colección, el valor de un `Optional` o lo que resuelve un `CompletableFuture`.
const RECEIVER_CALLBACKS: &[&str] = &[
    "map",
    "flatMap",
    "forEach",
    "forEachOrdered",
    "filter",
    "peek",
    "anyMatch",
    "allMatch",
    "noneMatch",
    "reduce",
    "ifPresent",
    "ifPresentOrElse",
    "thenApply",
    "thenAccept",
    "thenCompose",
    "replaceAll",
    "removeIf",
    "computeIfPresent",
    "mapToObj",
    "takeWhile",
    "dropWhile",
];
// Los métodos cuyo resultado es lo que devuelve su lambda.
const RESULT_CALLBACKS: &[&str] = &[
    "map",
    "flatMap",
    "mapToObj",
    "reduce",
    "thenApply",
    "thenCompose",
    "orElseGet",
    "computeIfAbsent",
    "computeIfPresent",
    "compute",
    "merge",
    "or",
];
const SQL_STATEMENTS: &[&str] = &["Statement", "PreparedStatement", "CallableStatement"];
const SQL_STATEMENT_METHODS: &[&str] = &[
    "execute",
    "executeQuery",
    "executeUpdate",
    "executeLargeUpdate",
    "addBatch",
];
const SQL_CONNECTIONS: &[&str] = &["Connection"];
const SQL_CONNECTION_METHODS: &[&str] = &["prepareStatement", "prepareCall", "nativeSQL"];
const SQL_TEMPLATES: &[&str] = &[
    "JdbcTemplate",
    "NamedParameterJdbcTemplate",
    "JdbcOperations",
    "NamedParameterJdbcOperations",
];
const SQL_TEMPLATE_METHODS: &[&str] = &[
    "query",
    "queryForObject",
    "queryForList",
    "queryForMap",
    "queryForRowSet",
    "queryForStream",
    "update",
    "batchUpdate",
    "execute",
];
const SQL_SESSIONS: &[&str] = &["EntityManager", "Session", "StatelessSession"];
const SQL_SESSION_METHODS: &[&str] = &["createQuery", "createNativeQuery", "createSQLQuery"];
const SQL_CLIENTS: &[&str] = &["JdbcClient"];
const SQL_CLIENT_METHODS: &[&str] = &["sql"];
// jOOQ con SQL en texto: el `DSLContext` y los `DSL.sql`, `DSL.field`... estáticos.
const JOOQ_CONTEXTS: &[&str] = &["DSLContext"];
const JOOQ_CONTEXT_METHODS: &[&str] = &[
    "fetch",
    "fetchOne",
    "fetchSingle",
    "fetchOptional",
    "fetchLazy",
    "fetchStream",
    "fetchValue",
    "execute",
    "query",
    "resultQuery",
];
const JOOQ_BUILDERS: &[&str] = &["DSL"];
const JOOQ_BUILDER_METHODS: &[&str] = &["sql", "field", "condition", "table", "query", "resultQuery"];
const COMMAND_RUNTIMES: &[&str] = &["Runtime"];
const COMMAND_RUNTIME_METHODS: &[&str] = &["exec"];
const COMMAND_BUILDERS: &[&str] = &["ProcessBuilder"];
const COMMAND_BUILDER_METHODS: &[&str] = &["command"];
const PATH_FACTORIES: &[&str] = &["Paths", "Path"];
const PATH_FACTORY_METHODS: &[&str] = &["get", "of"];
const PATH_CONSTRUCTORS: &[&str] = &[
    "File",
    "FileInputStream",
    "FileOutputStream",
    "FileReader",
    "FileWriter",
    "RandomAccessFile",
];
const COMMAND_CONSTRUCTORS: &[&str] = &["ProcessBuilder"];
const METHOD_SINKS: &[(&[&str], &[&str], SinkKind)] = &[
    (SQL_STATEMENTS, SQL_STATEMENT_METHODS, SinkKind::Sql),
    (SQL_CONNECTIONS, SQL_CONNECTION_METHODS, SinkKind::Sql),
    (SQL_TEMPLATES, SQL_TEMPLATE_METHODS, SinkKind::Sql),
    (SQL_SESSIONS, SQL_SESSION_METHODS, SinkKind::Sql),
    (SQL_CLIENTS, SQL_CLIENT_METHODS, SinkKind::Sql),
    (JOOQ_CONTEXTS, JOOQ_CONTEXT_METHODS, SinkKind::Sql),
    (JOOQ_BUILDERS, JOOQ_BUILDER_METHODS, SinkKind::Sql),
    (COMMAND_RUNTIMES, COMMAND_RUNTIME_METHODS, SinkKind::Command),
    (COMMAND_BUILDERS, COMMAND_BUILDER_METHODS, SinkKind::Command),
    (RESPONSE_WRITERS, RESPONSE_WRITER_METHODS, SinkKind::Html),
    (PATH_FACTORIES, PATH_FACTORY_METHODS, SinkKind::Path),
];
const RUNTIME_FACTORY: &str = "getRuntime";
const RUNTIME: &str = "Runtime";
// Lo que escribe en la respuesta HTTP: `response.getWriter()` o `response.getOutputStream()`.
const RESPONSE_TYPES: &[&str] = &[
    "HttpServletResponse",
    "ServletResponse",
    "HttpServletResponseWrapper",
];
const RESPONSE_STREAMS: &[&str] = &["getWriter", "getOutputStream"];
const RESPONSE_WRITER: &str = "ResponseWriter";
const RESPONSE_WRITERS: &[&str] = &[RESPONSE_WRITER];
const RESPONSE_WRITER_METHODS: &[&str] = &["print", "println", "printf", "format", "write", "append"];
// Los escapes de HTML de ESAPI, Spring, commons-text, commons-lang y el encoder de OWASP: lo que
// devuelven ya no sirve para un XSS.
const HTML_ESCAPES: &[&str] = &[
    "encodeForHTML",
    "encodeForHTMLAttribute",
    "encodeForJavaScript",
    "encodeForXML",
    "encodeForXMLAttribute",
    "htmlEscape",
    "escapeHtml",
    "escapeHtml3",
    "escapeHtml4",
    "escapeXml",
    "escapeXml10",
    "escapeXml11",
    "escapeEcmaScript",
    "forHtml",
    "forHtmlContent",
    "forHtmlAttribute",
    "forJavaScript",
];

pub fn is_source(receiver_type: &str, method: &str) -> bool {
    SOURCE_TYPES.contains(&receiver_type) && SOURCE_METHODS.contains(&method)
}

pub fn is_source_parameter(annotations: &[String]) -> bool {
    annotations
        .iter()
        .any(|annotation| SOURCE_PARAMETER_ANNOTATIONS.contains(&annotation.as_str()))
}

pub fn gives_clean_result(receiver: Option<&str>, method: &str) -> bool {
    CLEAN_RESULTS.contains(&method)
        || receiver
            .is_some_and(|written| NUMERIC_TYPES.contains(&written) && NUMERIC_CONVERSIONS.contains(&method))
}

pub fn passes_receiver_to_callback(method: &str) -> bool {
    RECEIVER_CALLBACKS.contains(&method)
}

pub fn keeps_callback_result(method: &str) -> bool {
    RESULT_CALLBACKS.contains(&method)
}

pub fn absorbs_arguments(method: &str) -> bool {
    ABSORBING_METHODS.contains(&method)
}

// Un método que recibe una consulta, un comando o una ruta, según el tipo de su receptor.
pub fn method_sink(receiver_type: &str, method: &str) -> Option<SinkKind> {
    METHOD_SINKS
        .iter()
        .find(|(types, methods, _)| types.contains(&receiver_type) && methods.contains(&method))
        .map(|(_, _, kind)| *kind)
}

pub fn might_be_method_sink(method: &str) -> bool {
    METHOD_SINKS
        .iter()
        .any(|(_, methods, _)| methods.contains(&method))
}

pub fn constructor_sink(created: &str) -> Option<SinkKind> {
    if PATH_CONSTRUCTORS.contains(&created) {
        Some(SinkKind::Path)
    } else if COMMAND_CONSTRUCTORS.contains(&created) {
        Some(SinkKind::Command)
    } else {
        None
    }
}

// `Runtime.getRuntime().exec(...)` y `response.getWriter().print(...)`: el receptor es lo que
// devuelve la fábrica.
pub fn factory_type(owner: &str, method: &str) -> Option<&'static str> {
    if owner == RUNTIME && method == RUNTIME_FACTORY {
        Some(RUNTIME)
    } else if RESPONSE_TYPES.contains(&owner) && RESPONSE_STREAMS.contains(&method) {
        Some(RESPONSE_WRITER)
    } else {
        None
    }
}

// Un parámetro de estos tipos no puede llevar texto armado por el usuario, aunque venga del pedido.
const CLEAN_PARAMETER_TYPES: &[&str] = &[
    "int",
    "long",
    "short",
    "byte",
    "double",
    "float",
    "boolean",
    "char",
    "Integer",
    "Long",
    "Short",
    "Byte",
    "Double",
    "Float",
    "Boolean",
    "Character",
    "UUID",
    "BigDecimal",
    "BigInteger",
    "LocalDate",
    "LocalDateTime",
    "LocalTime",
    "Instant",
    "OffsetDateTime",
    "ZonedDateTime",
    "Duration",
];
// Lo que deja sólo el nombre del archivo, sin carpetas: limpia una ruta.
const QUALIFIER_SEPARATOR: char = '.';
const PATH_CLEANERS: &[(&str, &str)] = &[("FilenameUtils", "getName"), ("FilenameUtils", "getBaseName")];

pub fn is_clean_type(written: &str) -> bool {
    let simple = written.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(written);
    CLEAN_PARAMETER_TYPES.contains(&simple)
}

pub fn cleans_path(owner: Option<&str>, method: &str) -> bool {
    owner.is_some_and(|written| PATH_CLEANERS.contains(&(written, method)))
}

pub fn escapes_html(method: &str) -> bool {
    HTML_ESCAPES.contains(&method)
}
