use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de Analytics.hs de ShellCheck sobre la forma de un comando: nombres que no pueden ser un comando, pipes y redirecciones que no llevan a nada, y elementos de arreglos mal escritos.
pub const COMMAND_SHAPES_TEXTS: [(ShellRule, RuleText); 36] = [
    (
        ShellRule::Sc2188,
        RuleText::new(
            "Redirección sin comando (SC2188 de ShellCheck)",
            "movela a su comando, o usá true si no tiene que hacer nada",
            "No corre nada, y el archivo se crea o se vacía.",
            "> file se vuelve true > file.",
        ),
    ),
    (
        ShellRule::Sc2189,
        RuleText::new(
            "| entre una redirección y su comando (SC2189 de ShellCheck)",
            "poné la redirección al lado del comando",
            "La redirección no se aplica a nada, y el comando lee un pipe vacío.",
            "< file | cmd se vuelve cmd < file.",
        ),
    ),
    (
        ShellRule::Sc2190,
        RuleText::new(
            "Elemento de arreglo asociativo sin índice (SC2190 de ShellCheck)",
            "usá ( [índice]=valor )",
            "Los arreglos asociativos no tienen posiciones: el elemento se pierde.",
            "declare -A a=(b) se vuelve declare -A a=([k]=b).",
        ),
    ),
    (
        ShellRule::Sc2191,
        RuleText::new(
            "= literal en un elemento de arreglo (SC2191 de ShellCheck)",
            "usá ( [índice]=valor ), o poné comillas",
            "Sin corchetes el = queda en el valor: no hay índice.",
            "a=(1=b) se vuelve a=([1]=b).",
        ),
    ),
    (
        ShellRule::Sc2192,
        RuleText::new(
            "Elemento de arreglo sin valor (SC2192 de ShellCheck)",
            "sacá los espacios después del =, o usá \"\"",
            "La palabra siguiente es otro elemento.",
            "a=([x]= b) se vuelve a=([x]=b).",
        ),
    ),
    (
        ShellRule::Sc2210,
        RuleText::new(
            "Redirección a un archivo con nombre de número (SC2210 de ShellCheck)",
            "usá -gt para comparar, o >&2 para un descriptor",
            "Crea un archivo que se llama 2.",
            "foo > 2 se vuelve foo >&2.",
        ),
    ),
    (
        ShellRule::Sc2211,
        RuleText::new(
            "Glob usado como nombre de comando (SC2211 de ShellCheck)",
            "ponelo en un arreglo o en ${..}, o entre comillas",
            "El primer archivo que coincida correría como comando.",
            "*.sh se vuelve ./run.sh.",
        ),
    ),
    (
        ShellRule::Sc2215,
        RuleText::new(
            "Opción usada como nombre de comando (SC2215 de ShellCheck)",
            "corregí el salto de línea, o agregá el [ .. ] que falta",
            "No hay ningún comando que se llame -e.",
            "-e file se vuelve [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2216,
        RuleText::new(
            "Pipe a un comando que no lee stdin (SC2216 de ShellCheck)",
            "usá cat o xargs",
            "El comando ignora lo que llega por el pipe.",
            "foo | rm se vuelve foo | xargs rm.",
        ),
    ),
    (
        ShellRule::Sc2217,
        RuleText::new(
            "Redirección a un comando que no lee stdin (SC2217 de ShellCheck)",
            "usá cat o xargs",
            "El comando ignora esa entrada.",
            "echo < file se vuelve cat file.",
        ),
    ),
    (
        ShellRule::Sc2238,
        RuleText::new(
            "Redirección a o desde el nombre de un comando (SC2238 de ShellCheck)",
            "usá un pipe o xargs, o poné comillas",
            "Se toma como un archivo con ese nombre.",
            "foo > grep se vuelve foo | grep.",
        ),
    ),
    (
        ShellRule::Sc2247,
        RuleText::new(
            "$\"(..)\" con el $ y la comilla al revés (SC2247 de ShellCheck)",
            "usá \"$(..)\"",
            "$\"..\" es un texto traducido: lo de adentro no corre.",
            "$\"(ls)\" se vuelve \"$(ls)\".",
        ),
    ),
    (
        ShellRule::Sc2256,
        RuleText::new(
            "$\"nombre\" cuando nombre es una variable (SC2256 de ShellCheck)",
            "usá \"$nombre\"",
            "$\"..\" es un texto traducido, no una expansión.",
            "$\"a\" se vuelve \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2259,
        RuleText::new(
            "Redirección que pisa la entrada del pipe (SC2259 de ShellCheck)",
            "unilas, o pasá nombres de archivo",
            "El comando lee el archivo e ignora el pipe.",
            "a | b < f necesita cat - f | b.",
        ),
    ),
    (
        ShellRule::Sc2260,
        RuleText::new(
            "Redirección que pisa la salida del pipe (SC2260 de ShellCheck)",
            "usá tee para mandarla a los dos",
            "La etapa siguiente del pipeline no recibe nada.",
            "a > f | b se vuelve a | tee f | b.",
        ),
    ),
    (
        ShellRule::Sc2261,
        RuleText::new(
            "Varias redirecciones compiten por el mismo descriptor (SC2261 de ShellCheck)",
            "usá cat, tee, o pasá nombres de archivo",
            "Sólo vale la última.",
            "a > f > g se vuelve a | tee f > g.",
        ),
    ),
    (
        ShellRule::Sc2262,
        RuleText::new(
            "Alias definido y usado en la misma unidad de lectura (SC2262 de ShellCheck)",
            "usá una función",
            "El shell lee toda la unidad antes de definir el alias.",
            "alias a=b; a se vuelve a() { b; }; a.",
        ),
    ),
    (
        ShellRule::Sc2263,
        RuleText::new(
            "Comando que no va a usar el alias recién definido (SC2263 de ShellCheck)",
            "usá una función",
            "El alias sólo existe desde la unidad de lectura siguiente.",
            "alias a=b; a se vuelve a() { b; }; a.",
        ),
    ),
    (
        ShellRule::Sc2264,
        RuleText::new(
            "Función que siempre se llama a sí misma (SC2264 de ShellCheck)",
            "agregá command delante del nombre",
            "Sin eso la función no termina nunca.",
            "ls() { ls -l; } se vuelve ls() { command ls -l; }.",
        ),
    ),
    (
        ShellRule::Sc2270,
        RuleText::new(
            "Asignación a un parámetro posicional (SC2270 de ShellCheck)",
            "usá set -- primero segundo, o [ ] para comparar",
            "El shell busca un comando con ese nombre.",
            "1=a se vuelve set -- a.",
        ),
    ),
    (
        ShellRule::Sc2271,
        RuleText::new(
            "Asignación a un nombre calculado (SC2271 de ShellCheck)",
            "usá arreglos, declare \"var$n=valor\" o read/eval",
            "El shell busca un comando con ese nombre.",
            "var$n=1 se vuelve declare \"var$n=1\".",
        ),
    ),
    (
        ShellRule::Sc2272,
        RuleText::new(
            "Nombre de comando con == (SC2272 de ShellCheck)",
            "usá [ \"$var\" = valor ]",
            "El shell busca un comando con ese nombre.",
            "$a==1 se vuelve [ \"$a\" = 1 ].",
        ),
    ),
    (
        ShellRule::Sc2273,
        RuleText::new(
            "Línea de ===== (SC2273 de ShellCheck)",
            "resolvé el conflicto de merge, o hacelo un comentario",
            "El shell intenta correrla como comando.",
            "======= se vuelve # =======.",
        ),
    ),
    (
        ShellRule::Sc2274,
        RuleText::new(
            "Nombre de comando que empieza con === (SC2274 de ShellCheck)",
            "hacé un comentario con el borde",
            "El shell intenta correrlo como comando.",
            "=== Título === se vuelve # === Título ===.",
        ),
    ),
    (
        ShellRule::Sc2275,
        RuleText::new(
            "Nombre de comando que empieza con = (SC2275 de ShellCheck)",
            "corregí el salto de línea",
            "El valor quedó separado de la variable.",
            "a\n=1 se vuelve a=1.",
        ),
    ),
    (
        ShellRule::Sc2276,
        RuleText::new(
            "Nombre de comando con = (SC2276 de ShellCheck)",
            "corregí la asignación o la comparación",
            "El shell busca un comando con ese nombre.",
            "\"a\"=1 se vuelve a=1.",
        ),
    ),
    (
        ShellRule::Sc2277,
        RuleText::new(
            "Asignación a $0 en bash (SC2277 de ShellCheck)",
            "usá BASH_ARGV0",
            "Esta forma busca un comando con ese nombre.",
            "0=a se vuelve BASH_ARGV0=a.",
        ),
    ),
    (
        ShellRule::Sc2278,
        RuleText::new(
            "Asignación a $0 en ksh (SC2278 de ShellCheck)",
            "usá otra variable",
            "$0 no se puede asignar en ksh.",
            "0=a usa otra variable.",
        ),
    ),
    (
        ShellRule::Sc2279,
        RuleText::new(
            "Asignación a $0 en dash o BusyBox (SC2279 de ShellCheck)",
            "usá otra variable",
            "Se vuelve un nombre de comando.",
            "0=a usa otra variable.",
        ),
    ),
    (
        ShellRule::Sc2280,
        RuleText::new(
            "Asignación a $0 en sh (SC2280 de ShellCheck)",
            "usá otra variable",
            "No hay forma portable de hacerlo.",
            "0=a usa otra variable.",
        ),
    ),
    (
        ShellRule::Sc2281,
        RuleText::new(
            "$ del lado izquierdo de una asignación (SC2281 de ShellCheck)",
            "sacá el $",
            "Expande la variable y busca un comando con ese nombre.",
            "$a=1 se vuelve a=1.",
        ),
    ),
    (
        ShellRule::Sc2282,
        RuleText::new(
            "Nombre de variable que empieza con un número (SC2282 de ShellCheck)",
            "usá un nombre que empiece con una letra",
            "Se lee como un comando.",
            "9a=1 se vuelve a9=1.",
        ),
    ),
    (
        ShellRule::Sc2286,
        RuleText::new(
            "Texto vacío como nombre de comando (SC2286 de ShellCheck)",
            "corregí la sintaxis, o usá true si no tiene que hacer nada",
            "El shell no puede correr un nombre vacío.",
            "\"\" a se vuelve true.",
        ),
    ),
    (
        ShellRule::Sc2287,
        RuleText::new(
            "Nombre de comando terminado en / (SC2287 de ShellCheck)",
            "corregí la sintaxis",
            "Es una carpeta, no un programa.",
            "foo/ se vuelve foo.",
        ),
    ),
    (
        ShellRule::Sc2288,
        RuleText::new(
            "Nombre de comando terminado en un símbolo (SC2288 de ShellCheck)",
            "corregí la sintaxis",
            "Esos símbolos suelen ser un error o un espacio que falta.",
            "foo, se vuelve foo.",
        ),
    ),
    (
        ShellRule::Sc2289,
        RuleText::new(
            "Nombre de comando con un tab o un salto de línea (SC2289 de ShellCheck)",
            "corregí la sintaxis",
            "Probablemente una comilla quedó fuera de lugar.",
            "\"foo\nbar\" se vuelve foo.",
        ),
    ),
];
