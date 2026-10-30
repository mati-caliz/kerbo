use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Los chequeos de Analytics.hs de ShellCheck que no piden flujo de datos, y los source que su parser no sigue.
pub const ANALYTICS_TEXTS: [(ShellRule, RuleText); 24] = [
    (
        ShellRule::Sc1090,
        RuleText::new(
            "source de una ruta que ShellCheck no puede conocer (SC1090 de ShellCheck)",
            "agregá una directiva # shellcheck source=ruta, o source=/dev/null si no importa",
            "Sin una ruta constante el archivo cargado no se revisa, y las variables que define parecen sin definir.",
            "source \"$1\" lleva # shellcheck source=lib.sh antes.",
        ),
    ),
    (
        ShellRule::Sc1091,
        RuleText::new(
            "source de un archivo que no se revisa con el script (SC1091 de ShellCheck)",
            "pasá también el archivo a la revisión, o agregá # shellcheck source=/dev/null",
            "El archivo cargado no se sigue, así que lo que define es desconocido para el resto de los chequeos.",
            "source ./lib.sh se revisa junto con lib.sh, o lleva source=/dev/null.",
        ),
    ),
    (
        ShellRule::Sc2007,
        RuleText::new(
            "Aritmética $[..], que está deprecada (SC2007 de ShellCheck)",
            "usá $((..))",
            "$[..] es una forma vieja de bash que las shells nuevas pueden sacar.",
            "$[a+1] se vuelve $((a+1)).",
        ),
    ),
    (
        ShellRule::Sc2009,
        RuleText::new(
            "ps pasado a grep (SC2009 de ShellCheck)",
            "usá pgrep",
            "grep también coincide con su propio proceso y con líneas por casualidad; pgrep busca por nombre de proceso.",
            "ps aux | grep nginx se vuelve pgrep -a nginx.",
        ),
    ),
    (
        ShellRule::Sc2010,
        RuleText::new(
            "ls pasado a grep (SC2010 de ShellCheck)",
            "usá un glob o un loop con una condición",
            "Leer la salida de ls se rompe con nombres que tienen espacios, saltos de línea o caracteres raros.",
            "ls | grep '\\.log$' se vuelve for f in *.log; do ...; done.",
        ),
    ),
    (
        ShellRule::Sc2011,
        RuleText::new(
            "ls pasado a xargs (SC2011 de ShellCheck)",
            "usá find -print0 | xargs -0 o find -exec",
            "xargs parte la salida de ls en los espacios, así que los nombres con espacios se vuelven varios argumentos.",
            "ls | xargs rm se vuelve find . -maxdepth 1 -type f -exec rm {} +.",
        ),
    ),
    (
        ShellRule::Sc2012,
        RuleText::new(
            "Salida de ls leída por otro comando (SC2012 de ShellCheck)",
            "usá find o un glob",
            "La salida de ls es para personas: los nombres con saltos de línea o caracteres raros no sobreviven.",
            "ls *.txt | wc -l se vuelve find . -name '*.txt' | wc -l, o un glob en un arreglo.",
        ),
    ),
    (
        ShellRule::Sc2013,
        RuleText::new(
            "Loop for sobre $(cat archivo), que parte en palabras (SC2013 de ShellCheck)",
            "usá while IFS= read -r linea; do ...; done < archivo",
            "for parte la salida en los espacios y expande globs, así que una línea con espacios se vuelve varios elementos.",
            "for l in $(cat f) se vuelve while IFS= read -r l; do ...; done < f.",
        ),
    ),
    (
        ShellRule::Sc2026,
        RuleText::new(
            "Palabra que quedó afuera de las comillas simples entre dos partes (SC2026 de ShellCheck)",
            "anidá las comillas como '\"'comillas simples'\"' o poné también la palabra entre comillas",
            "La palabra entre las dos partes con comillas simples queda afuera y se parte y expande.",
            "echo 'es'tá'bien' deja tá afuera de las comillas.",
        ),
    ),
    (
        ShellRule::Sc2027,
        RuleText::new(
            "Expansión entre dos partes con comillas dobles, que queda sin comillas (SC2027 de ShellCheck)",
            "sacá las comillas de adentro o escapalas",
            "\"a\"$x\"b\" cierra las comillas antes de $x, así que $x se parte y se expande.",
            "echo \"Hola \"$nombre\"!\" se vuelve echo \"Hola $nombre!\".",
        ),
    ),
    (
        ShellRule::Sc2038,
        RuleText::new(
            "find pasado a xargs sin separador nulo (SC2038 de ShellCheck)",
            "usá find -print0 | xargs -0, o find -exec ... +",
            "xargs parte en los espacios y saltos de línea, así que los nombres que los tienen se rompen.",
            "find . -name '*.log' | xargs rm se vuelve find . -name '*.log' -exec rm {} +.",
        ),
    ),
    (
        ShellRule::Sc2048,
        RuleText::new(
            "$* o ${arreglo[*]} sin comillas (SC2048 de ShellCheck)",
            "usá \"$@\" o \"${arreglo[@]}\" con comillas",
            "Sin comillas, cada argumento se vuelve a partir en los espacios y se expande.",
            "cmd $* se vuelve cmd \"$@\".",
        ),
    ),
    (
        ShellRule::Sc2070,
        RuleText::new(
            "[ -n ] con un argumento sin comillas (SC2070 de ShellCheck)",
            "poné el argumento entre comillas o usá [[ ]]",
            "Si la variable está vacía, [ -n ] mira el string -n y siempre da verdadero.",
            "[ -n $x ] se vuelve [ -n \"$x\" ].",
        ),
    ),
    (
        ShellRule::Sc2084,
        RuleText::new(
            "$((expr)) como comando entero, que ejecuta el resultado (SC2084 de ShellCheck)",
            "sacá el $ o asignalo con _=$((expr))",
            "La shell corre el número que sale como si fuera un comando.",
            "$((contador++)) se vuelve ((contador++)) o : $((contador++)).",
        ),
    ),
    (
        ShellRule::Sc2088,
        RuleText::new(
            "~ entre comillas, donde no se expande (SC2088 de ShellCheck)",
            "usá $HOME",
            "La tilde sólo se expande sin comillas al principio de una palabra.",
            "\"~/backup\" se vuelve \"$HOME/backup\".",
        ),
    ),
    (
        ShellRule::Sc2091,
        RuleText::new(
            "$(cmd) como comando entero, que ejecuta su salida (SC2091 de ShellCheck)",
            "sacá el $( ), o usá eval si la idea es correr la salida",
            "La salida del comando se corre como otro comando.",
            "if $(grep -q x f); then se vuelve if grep -q x f; then.",
        ),
    ),
    (
        ShellRule::Sc2092,
        RuleText::new(
            "`cmd` como comando entero, que ejecuta su salida (SC2092 de ShellCheck)",
            "sacá los backticks, o usá eval si la idea es correr la salida",
            "La salida del comando se corre como otro comando.",
            "`which python` se vuelve which python.",
        ),
    ),
    (
        ShellRule::Sc2119,
        RuleText::new(
            "Llamada sin argumentos a una función que lee $1 (SC2119 de ShellCheck)",
            "pasale los argumentos: nombre \"$@\" si tiene que ver los del script",
            "Dentro de una función $1 es el primer argumento de la función, no el del script.",
            "uso() { echo \"$1\"; }; uso se vuelve uso \"$@\".",
        ),
    ),
    (
        ShellRule::Sc2120,
        RuleText::new(
            "Función que lee sus argumentos pero nunca se llama con ninguno (SC2120 de ShellCheck)",
            "pasale los argumentos en las llamadas, o usá los valores del script de otra forma",
            "Cada llamada deja $1 vacío, que suele ser un argumento olvidado o una confusión con el $1 del script.",
            "saludar() { echo \"$1\"; }; saludar se vuelve saludar \"$nombre\".",
        ),
    ),
    (
        ShellRule::Sc2124,
        RuleText::new(
            "Arreglo asignado a una variable común (SC2124 de ShellCheck)",
            "asigná un arreglo con ( ), o usá * en vez de @ para juntar",
            "var=$@ junta los elementos y pierde dónde terminaba cada uno.",
            "args=$@ se vuelve args=(\"$@\").",
        ),
    ),
    (
        ShellRule::Sc2125,
        RuleText::new(
            "Glob o expansión de llaves en una asignación, donde queda literal (SC2125 de ShellCheck)",
            "ponelo entre comillas, o usá un arreglo para expandirlo",
            "Las asignaciones no expanden globs ni llaves, así que la variable guarda el patrón.",
            "archivos=*.txt se vuelve archivos=(*.txt).",
        ),
    ),
    (
        ShellRule::Sc2126,
        RuleText::new(
            "grep pasado a wc -l (SC2126 de ShellCheck)",
            "usá grep -c",
            "grep cuenta las líneas que coinciden por su cuenta.",
            "grep x f | wc -l se vuelve grep -c x f.",
        ),
    ),
    (
        ShellRule::Sc2140,
        RuleText::new(
            "Palabra de la forma \"A\"B\"C\", con B afuera de las comillas (SC2140 de ShellCheck)",
            "escribí \"ABC\", o escapá las comillas de adentro como \\\"",
            "La parte del medio queda sin comillas, y casi nunca es lo que se quería.",
            "echo \"a\"b\"c\" se vuelve echo \"abc\" o echo \"a\\\"b\\\"c\".",
        ),
    ),
    (
        ShellRule::Sc2145,
        RuleText::new(
            "Argumento que mezcla un string y un arreglo (SC2145 de ShellCheck)",
            "usá * para juntar, o pasá el arreglo como argumento aparte",
            "\"texto $@\" pega el texto al primer elemento y separa el resto en palabras.",
            "echo \"Args: $@\" se vuelve echo \"Args: $*\".",
        ),
    ),
];
