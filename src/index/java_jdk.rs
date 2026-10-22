// Lo que kerbo sabe del JDK sin leerlo: no viene en el classpath de Maven, y `ct.sym` no trae las
// anotaciones ni el detalle que hace falta. Tablas chicas, de lo que aparece en el código de todos
// los días.

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
    "java.util.Scanner",
    "java.nio.channels.FileChannel",
    "java.nio.channels.SocketChannel",
    "java.nio.file.DirectoryStream",
    "java.nio.file.FileSystem",
    "java.nio.file.WatchService",
];

// Lo que se cierra pero IntelliJ no pide cerrar: no toma nada del sistema.
pub const NOT_RESOURCES: &[&str] = &[
    "java.io.ByteArrayInputStream",
    "java.io.ByteArrayOutputStream",
    "java.io.CharArrayReader",
    "java.io.CharArrayWriter",
    "java.io.StringReader",
    "java.io.StringWriter",
    "java.util.stream.Stream",
    "java.util.stream.IntStream",
    "java.util.stream.LongStream",
    "java.util.stream.DoubleStream",
];

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
