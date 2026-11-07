# Cómo está armado

Las métricas se escriben una sola vez sobre el árbol de [tree-sitter](https://tree-sitter.github.io/).
Lo propio de cada lenguaje vive en `src/profile/`: qué nodo es una función, una decisión o un
operador lógico, y cómo cuenta líneas la herramienta que se imita. Los archivos se analizan en
paralelo y de a uno, sin cargar el programa entero.

La caché (`src/cache/`) guarda por archivo lo que mide el análisis, no los problemas: la clave es el
contenido, el lenguaje, los umbrales efectivos de ese archivo y la identidad del binario (versión,
tamaño y fecha), y la ruta queda afuera, así un archivo movido o el mismo archivo en otro clone no
se vuelve a analizar. Las reglas y los duplicados entre archivos se calculan siempre sobre lo
guardado. Está partida en 256 archivos por el primer byte de la clave, cada uno con su tope y
desalojo del menos usado; cada entrada va comprimida con LZ4 y con su suma de control, y se
escribe a un temporal que se renombra. En Rust el resultado de un archivo depende también de las
`macro_rules!`, así que la clave suma una huella de los archivos del crate que definen macros, las
traen con `#[macro_use]` o invocan una que arma funciones, de la versión de cada dependencia y de
la ruta del archivo: cambiar una macro vuelve a analizar a quien la usa, y editar una función
común, no.

Los textos del CLI (reporte, `explain`, errores, hook, MCP y LSP) viven en un catálogo tipado en
`src/i18n/`, uno por idioma; el compilador asegura que cada mensaje exista en los dos.

Un arreglo mecánico (`src/fix.rs`) es una lista de ediciones de rango (línea y columna de inicio y
de fin y el texto nuevo) más el bloque de diff unificado que resulta, calculado durante el análisis
y guardado en la caché con el resto. El LSP manda las ediciones como acción rápida y `--format
agent` manda el diff; una regla nueva sólo tiene que armar el `Fix` (borrar un nodo o insertar
antes de uno) y le llegan solos `kerbo fix`, `kerbo_fix` y el "arreglar todo" del LSP, que eligen
los arreglos que no se pisan con `fix::compatible`.

Los comentarios que silencian una herramienta original están en un solo registro,
`SYNTAXES` de `src/suppressions.rs`: el comentario (`--` o `#`), el marcador y si vale para la
línea siguiente o para el archivo. De ahí salen tanto la antitrampa como lo que se ignora al
reportar; los códigos se comparan con el nombre de la regla sin distinguir mayúsculas, así que una
herramienta nueva sólo suma su fila.

## Desarrollo

El gate (formato, clippy en modo estricto, techos de tamaño, tests con cobertura y `cargo-deny`)
es `scripts/verify-rust.sh` y corre en un contenedor, así que sólo hace falta Docker. En Windows,
desde el bash de Git, va con `MSYS_NO_PATHCONV=1 bash scripts/verify-rust.sh`: sin eso Git Bash
convierte las rutas que se le pasan a `docker run`. El `.gitattributes` deja los archivos de texto
con LF aunque `core.autocrlf` esté prendido, así `cargo fmt --check` no falla por los finales de
línea.
