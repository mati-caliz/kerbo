// Lo que kerbo sabe del JDK sin leerlo. `CLOSEABLE` y `RETURNS` sólo hacen falta si no se encontró
// un JDK; los nulos de `METHODS` vienen de IntelliJ, porque `ct.sym` no trae anotaciones.

// Lo que se cierra: las interfaces raíz y las clases e interfaces más usadas que las implementan.
pub const CLOSEABLE: &[&str] = &[
    "java.lang.AutoCloseable",
    "java.io.Closeable",
    "java.io.InputStream",
    "java.io.OutputStream",
    "java.io.Reader",
    "java.io.Writer",
    "java.io.FileInputStream",
    "java.io.FileOutputStream",
    "java.io.BufferedReader",
    "java.io.BufferedWriter",
    "java.io.PrintWriter",
    "java.io.ObjectInputStream",
    "java.io.ObjectOutputStream",
    "java.sql.Connection",
    "java.sql.Statement",
    "java.sql.PreparedStatement",
    "java.sql.CallableStatement",
    "java.sql.ResultSet",
    "java.util.concurrent.ExecutorService",
    "java.util.concurrent.ScheduledExecutorService",
    "java.net.http.HttpClient",
    "java.net.Socket",
    "java.net.ServerSocket",
    "java.util.zip.ZipFile",
    "java.util.jar.JarFile",
    "java.nio.channels.FileChannel",
    "java.nio.channels.SocketChannel",
    "java.nio.file.DirectoryStream",
    "java.nio.file.FileSystem",
    "java.nio.file.WatchService",
];

// Lo que se cierra pero IntelliJ no pide cerrar (sus `ignoredTypes` por defecto), con sus subtipos.
pub const NOT_RESOURCES: &[&str] = &[
    "java.util.stream.Stream",
    "java.util.stream.IntStream",
    "java.util.stream.LongStream",
    "java.util.stream.DoubleStream",
    "java.io.ByteArrayOutputStream",
    "java.io.ByteArrayInputStream",
    "java.io.StringBufferInputStream",
    "java.io.CharArrayWriter",
    "java.io.CharArrayReader",
    "java.io.StringWriter",
    "java.io.StringReader",
    "java.util.Formatter",
    "java.util.Scanner",
    "org.springframework.context.ConfigurableApplicationContext",
    "io.micronaut.context.ApplicationContext",
    "org.apache.pekko.actor.ActorSystem",
];

// Los `Stream` que tienen un archivo o un directorio abierto: aunque `Stream` se ignore, éstos sí.
pub const STREAM_HOLDERS: (&str, &[&str]) = ("java.nio.file.Files", &["lines", "walk", "list", "find"]);

// Qué devuelve un método del JDK que crea un recurso: (clase, método, clase devuelta).
pub const RETURNS: &[(&str, &str, &str)] = &[
    (
        "java.util.concurrent.Executors",
        "newFixedThreadPool",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newCachedThreadPool",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newSingleThreadExecutor",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newWorkStealingPool",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newVirtualThreadPerTaskExecutor",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newThreadPerTaskExecutor",
        "java.util.concurrent.ExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newScheduledThreadPool",
        "java.util.concurrent.ScheduledExecutorService",
    ),
    (
        "java.util.concurrent.Executors",
        "newSingleThreadScheduledExecutor",
        "java.util.concurrent.ScheduledExecutorService",
    ),
    (
        "java.net.http.HttpClient",
        "newHttpClient",
        "java.net.http.HttpClient",
    ),
    ("java.nio.file.Files", "newInputStream", "java.io.InputStream"),
    ("java.nio.file.Files", "newOutputStream", "java.io.OutputStream"),
    (
        "java.nio.file.Files",
        "newBufferedReader",
        "java.io.BufferedReader",
    ),
    (
        "java.nio.file.Files",
        "newBufferedWriter",
        "java.io.BufferedWriter",
    ),
    ("java.sql.DriverManager", "getConnection", "java.sql.Connection"),
    ("javax.sql.DataSource", "getConnection", "java.sql.Connection"),
    (
        "java.sql.Connection",
        "prepareStatement",
        "java.sql.PreparedStatement",
    ),
    ("java.sql.Connection", "createStatement", "java.sql.Statement"),
    ("java.sql.Connection", "prepareCall", "java.sql.CallableStatement"),
    ("java.sql.Statement", "executeQuery", "java.sql.ResultSet"),
    ("java.sql.PreparedStatement", "executeQuery", "java.sql.ResultSet"),
    ("java.lang.Class", "getResourceAsStream", "java.io.InputStream"),
    (
        "java.lang.ClassLoader",
        "getResourceAsStream",
        "java.io.InputStream",
    ),
    ("java.net.URL", "openStream", "java.io.InputStream"),
];

pub fn returned_by(class: &str, method: &str) -> Option<&'static str> {
    RETURNS
        .iter()
        .find(|(owner, name, _)| *owner == class && *name == method)
        .map(|(_, _, returned)| *returned)
}

const OBJECT: &str = "java.lang.Object";

// Lo que devuelven los métodos del JDK que aparecen en una cadena, y si IntelliJ los da por
// posiblemente null: el recurso que no está, la propiedad o la variable de entorno que no existe.
// El padre de un `File` no: Qodana no lo marca nunca.
const METHODS: &[(&str, &str, usize, &str, bool)] = &[
    (OBJECT, "getClass", 0, "java.lang.Class", false),
    (
        "java.lang.Class",
        "getResourceAsStream",
        1,
        "java.io.InputStream",
        true,
    ),
    ("java.lang.Class", "getResource", 1, "java.net.URL", true),
    (
        "java.lang.ClassLoader",
        "getResourceAsStream",
        1,
        "java.io.InputStream",
        true,
    ),
    ("java.lang.ClassLoader", "getResource", 1, "java.net.URL", true),
    ("java.lang.Thread", "currentThread", 0, "java.lang.Thread", false),
    (
        "java.lang.Thread",
        "getContextClassLoader",
        0,
        "java.lang.ClassLoader",
        false,
    ),
    ("java.lang.System", "getProperty", 1, "java.lang.String", true),
    ("java.lang.System", "getenv", 1, "java.lang.String", true),
    ("java.io.File", "getParentFile", 0, "java.io.File", false),
    ("java.io.File", "getParent", 0, "java.lang.String", false),
    ("java.io.File", "listFiles", 0, "java.io.File[]", true),
    ("java.io.File", "list", 0, "java.lang.String[]", true),
    ("java.util.Queue", "peek", 0, OBJECT, true),
    ("java.util.Queue", "poll", 0, OBJECT, true),
];

// Las propiedades que la JVM siempre define: `System.getProperty` de una de ellas no es null.
const SYSTEM: &str = "java.lang.System";
const GET_PROPERTY: &str = "getProperty";
const ALWAYS_DEFINED: &[&str] = &[
    "java.version",
    "java.version.date",
    "java.vendor",
    "java.vendor.url",
    "java.vendor.version",
    "java.home",
    "java.vm.specification.version",
    "java.vm.specification.vendor",
    "java.vm.specification.name",
    "java.vm.version",
    "java.vm.vendor",
    "java.vm.name",
    "java.specification.version",
    "java.specification.maintenance.version",
    "java.specification.vendor",
    "java.specification.name",
    "java.class.version",
    "java.class.path",
    "java.library.path",
    "java.io.tmpdir",
    "os.name",
    "os.arch",
    "os.version",
    "file.separator",
    "path.separator",
    "line.separator",
    "user.name",
    "user.home",
    "user.dir",
    "native.encoding",
    "stdout.encoding",
    "stderr.encoding",
];

pub fn is_always_defined(classes: &[String], name: &str, arguments: &[String]) -> bool {
    name == GET_PROPERTY
        && classes.iter().any(|class| class == SYSTEM)
        && matches!(arguments, [property] if ALWAYS_DEFINED.contains(&property.as_str()))
}

// Un método del JDK de alguna de esas clases (o de `Object`, que heredan todas): qué devuelve y si
// puede ser null.
pub fn jdk_method(classes: &[String], name: &str, arity: usize) -> Option<(&'static str, bool)> {
    classes
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(OBJECT))
        .find_map(|class| {
            METHODS
                .iter()
                .find(|(owner, method, count, _, _)| *owner == class && *method == name && *count == arity)
                .map(|(_, _, _, returned, nullable)| (*returned, *nullable))
        })
}
