use super::BASHISM_WHY;
use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Las construcciones de checkBashisms de ShellCheck.
pub const BASHISMS_TEXTS: [(ShellRule, RuleText); 33] = [
    (
        ShellRule::Sc3001,
        RuleText::new(
            "Sustitución de procesos <(...) en un script de sh (SC3001 de ShellCheck)",
            "usá un archivo temporal o un pipe",
            BASHISM_WHY,
            "diff <(a) <(b) se vuelve a > x; b > y; diff x y.",
        ),
    ),
    (
        ShellRule::Sc3002,
        RuleText::new(
            "Patrón extglob en un script de sh (SC3002 de ShellCheck)",
            "usá un case o varios globs comunes",
            BASHISM_WHY,
            "rm !(*.hs) se vuelve un loop que saltea *.hs.",
        ),
    ),
    (
        ShellRule::Sc3003,
        RuleText::new(
            "String $'...' en un script de sh (SC3003 de ShellCheck)",
            "usá printf para armar los caracteres especiales",
            BASHISM_WHY,
            "tab=$'\\t' se vuelve tab=$(printf '\\t').",
        ),
    ),
    (
        ShellRule::Sc3004,
        RuleText::new(
            "String traducido $\"...\" en un script de sh (SC3004 de ShellCheck)",
            "usá un string común entre comillas dobles, o gettext",
            BASHISM_WHY,
            "echo $\"Hola\" se vuelve echo \"Hola\".",
        ),
    ),
    (
        ShellRule::Sc3005,
        RuleText::new(
            "Loop aritmético for ((...)) en un script de sh (SC3005 de ShellCheck)",
            "usá un while con un contador",
            BASHISM_WHY,
            "for ((i=0; i<3; i++)) se vuelve i=0; while [ $i -lt 3 ]; do ...; i=$((i+1)); done.",
        ),
    ),
    (
        ShellRule::Sc3006,
        RuleText::new(
            "Aritmética suelta ((...)) en un script de sh (SC3006 de ShellCheck)",
            "usá [ $((...)) -ne 0 ] o una asignación con $((...))",
            BASHISM_WHY,
            "((n++)) se vuelve n=$((n+1)).",
        ),
    ),
    (
        ShellRule::Sc3007,
        RuleText::new(
            "Aritmética $[...] en un script de sh (SC3007 de ShellCheck)",
            "usá $((...))",
            BASHISM_WHY,
            "$[a+1] se vuelve $((a+1)).",
        ),
    ),
    (
        ShellRule::Sc3008,
        RuleText::new(
            "Loop select en un script de sh (SC3008 de ShellCheck)",
            "mostrá las opciones y leé la elección con read",
            BASHISM_WHY,
            "select x in a b se vuelve una lista numerada y read opcion.",
        ),
    ),
    (
        ShellRule::Sc3009,
        RuleText::new(
            "Expansión de llaves {a,b} o {1..3} en un script de sh (SC3009 de ShellCheck)",
            "escribí las palabras completas o usá un loop",
            BASHISM_WHY,
            "cp f{,.bak} se vuelve cp f f.bak.",
        ),
    ),
    (
        ShellRule::Sc3010,
        RuleText::new(
            "Condición [[ ]] en un script de sh (SC3010 de ShellCheck)",
            "usá [ ] con las variables entre comillas, o case para los patrones",
            BASHISM_WHY,
            "[[ $a == b* ]] se vuelve case $a in b*) ...;; esac.",
        ),
    ),
    (
        ShellRule::Sc3011,
        RuleText::new(
            "Here-string <<< en un script de sh (SC3011 de ShellCheck)",
            "usá un pipe desde printf o un here-document",
            BASHISM_WHY,
            "grep x <<< \"$s\" se vuelve printf '%s\\n' \"$s\" | grep x.",
        ),
    ),
    (
        ShellRule::Sc3012,
        RuleText::new(
            "< o > lexicográfico en una condición en un script de sh (SC3012 de ShellCheck)",
            "compará con expr, o ordená los dos valores",
            BASHISM_WHY,
            "[ \"$a\" \\< \"$b\" ] se vuelve [ \"$(printf '%s\\n' \"$a\" \"$b\" | sort | head -n1)\" = \"$a\" ].",
        ),
    ),
    (
        ShellRule::Sc3014,
        RuleText::new(
            "== en una condición en vez de = en un script de sh (SC3014 de ShellCheck)",
            "usá un solo =",
            BASHISM_WHY,
            "[ \"$a\" == b ] se vuelve [ \"$a\" = b ].",
        ),
    ),
    (
        ShellRule::Sc3015,
        RuleText::new(
            "Comparación con regex =~ en un script de sh (SC3015 de ShellCheck)",
            "usá grep -E, expr o case",
            BASHISM_WHY,
            "[[ $a =~ ^[0-9]+$ ]] se vuelve printf '%s' \"$a\" | grep -Eq '^[0-9]+$'.",
        ),
    ),
    (
        ShellRule::Sc3016,
        RuleText::new(
            "test -v en un script de sh (SC3016 de ShellCheck)",
            "usá [ -n \"${var+x}\" ]",
            BASHISM_WHY,
            "[ -v nombre ] se vuelve [ -n \"${nombre+x}\" ].",
        ),
    ),
    (
        ShellRule::Sc3017,
        RuleText::new(
            "-a unario en una condición en un script de sh (SC3017 de ShellCheck)",
            "usá -e",
            BASHISM_WHY,
            "[ -a archivo ] se vuelve [ -e archivo ].",
        ),
    ),
    (
        ShellRule::Sc3018,
        RuleText::new(
            "++ o -- en aritmética en un script de sh (SC3018 de ShellCheck)",
            "usá un + 1 o - 1 explícito",
            BASHISM_WHY,
            "$((i++)) se vuelve i=$((i+1)).",
        ),
    ),
    (
        ShellRule::Sc3019,
        RuleText::new(
            "Potencia ** en aritmética en un script de sh (SC3019 de ShellCheck)",
            "multiplicá, o calculalo con awk o bc",
            BASHISM_WHY,
            "$((2**8)) se vuelve 256 o $(echo '2^8' | bc).",
        ),
    ),
    (
        ShellRule::Sc3020,
        RuleText::new(
            "Redirección &> en un script de sh (SC3020 de ShellCheck)",
            "redirigí stdout y después stderr: >archivo 2>&1",
            BASHISM_WHY,
            "cmd &> log se vuelve cmd > log 2>&1.",
        ),
    ),
    (
        ShellRule::Sc3021,
        RuleText::new(
            ">& a un nombre de archivo en un script de sh (SC3021 de ShellCheck)",
            "usá >archivo 2>&1",
            BASHISM_WHY,
            "cmd >& log se vuelve cmd > log 2>&1.",
        ),
    ),
    (
        ShellRule::Sc3022,
        RuleText::new(
            "Descriptor con nombre {fd}> en un script de sh (SC3022 de ShellCheck)",
            "elegí un número de descriptor fijo entre 3 y 9",
            BASHISM_WHY,
            "exec {fd}> log se vuelve exec 3> log.",
        ),
    ),
    (
        ShellRule::Sc3023,
        RuleText::new(
            "Descriptor de archivo mayor que 9 en un script de sh (SC3023 de ShellCheck)",
            "usá un descriptor entre 0 y 9",
            BASHISM_WHY,
            "exec 10> log se vuelve exec 9> log.",
        ),
    ),
    (
        ShellRule::Sc3024,
        RuleText::new(
            "Asignación con += en un script de sh (SC3024 de ShellCheck)",
            "concatená explícitamente",
            BASHISM_WHY,
            "path+=:/opt se vuelve path=\"$path:/opt\".",
        ),
    ),
    (
        ShellRule::Sc3025,
        RuleText::new(
            "Redirección a /dev/tcp o /dev/udp en un script de sh (SC3025 de ShellCheck)",
            "usá nc, curl u otra herramienta",
            BASHISM_WHY,
            "cat < /dev/tcp/host/80 se vuelve nc host 80.",
        ),
    ),
    (
        ShellRule::Sc3026,
        RuleText::new(
            "^ para negar una clase de glob en un script de sh (SC3026 de ShellCheck)",
            "usá ! en vez de ^",
            BASHISM_WHY,
            "ls [^a]* se vuelve ls [!a]*.",
        ),
    ),
    (
        ShellRule::Sc3028,
        RuleText::new(
            "Variable que sólo arma bash en un script de sh (SC3028 de ShellCheck)",
            "calculá el valor de otra forma, o pasá el script a bash",
            BASHISM_WHY,
            "echo $RANDOM se vuelve awk 'BEGIN{srand(); print int(rand()*32768)}'.",
        ),
    ),
    (
        ShellRule::Sc3029,
        RuleText::new(
            "Pipe |& en un script de sh (SC3029 de ShellCheck)",
            "usá 2>&1 |",
            BASHISM_WHY,
            "cmd |& tee log se vuelve cmd 2>&1 | tee log.",
        ),
    ),
    (
        ShellRule::Sc3030,
        RuleText::new(
            "Arreglo en un script de sh (SC3030 de ShellCheck)",
            "usá los parámetros posicionales con set --, o un string con separador",
            BASHISM_WHY,
            "archivos=(a b) se vuelve set -- a b.",
        ),
    ),
    (
        ShellRule::Sc3031,
        RuleText::new(
            "Redirección a o desde un glob en un script de sh (SC3031 de ShellCheck)",
            "escribí el nombre del archivo, o recorré lo que coincide",
            BASHISM_WHY,
            "cat > out*.log se vuelve cat > out.log.",
        ),
    ),
    (
        ShellRule::Sc3032,
        RuleText::new(
            "coproc en un script de sh (SC3032 de ShellCheck)",
            "usá un pipe con nombre (mkfifo) y un proceso en segundo plano",
            BASHISM_WHY,
            "coproc worker se vuelve mkfifo fifo; worker < fifo &.",
        ),
    ),
    (
        ShellRule::Sc3033,
        RuleText::new(
            "Nombre de función fuera de [a-zA-Z_][a-zA-Z0-9_]* en un script de sh (SC3033 de ShellCheck)",
            "renombrá la función con letras, dígitos y guiones bajos",
            BASHISM_WHY,
            "foo-bar() se vuelve foo_bar().",
        ),
    ),
    (
        ShellRule::Sc3034,
        RuleText::new(
            "$(<archivo) para leer un archivo en un script de sh (SC3034 de ShellCheck)",
            "usá $(cat archivo)",
            BASHISM_WHY,
            "texto=$(<archivo) se vuelve texto=$(cat archivo).",
        ),
    ),
    (
        ShellRule::Sc3035,
        RuleText::new(
            "`<archivo` para leer un archivo en un script de sh (SC3035 de ShellCheck)",
            "usá $(cat archivo)",
            BASHISM_WHY,
            "texto=`<archivo` se vuelve texto=$(cat archivo).",
        ),
    ),
];
