use super::BASHISM_WHY;
use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Los comandos y opciones de checkBashisms de ShellCheck.
pub const BASHISM_COMMANDS_TEXTS: [(ShellRule, RuleText); 31] = [
    (
        ShellRule::Sc3036,
        RuleText::new(
            "Opción de echo que el shell no tiene en un script de sh (SC3036 de ShellCheck)",
            "usá printf",
            BASHISM_WHY,
            "echo -e 'a\\tb' se vuelve printf 'a\\tb\\n'.",
        ),
    ),
    (
        ShellRule::Sc3037,
        RuleText::new(
            "Opciones de echo en un script de sh (SC3037 de ShellCheck)",
            "usá printf, que se comporta igual en todos lados",
            BASHISM_WHY,
            "echo -n \"$x\" se vuelve printf '%s' \"$x\".",
        ),
    ),
    (
        ShellRule::Sc3038,
        RuleText::new(
            "Opciones de exec en un script de sh (SC3038 de ShellCheck)",
            "corré el comando sin las opciones de exec, o pasá a bash",
            BASHISM_WHY,
            "exec -a nombre cmd se vuelve exec cmd.",
        ),
    ),
    (
        ShellRule::Sc3039,
        RuleText::new(
            "let en un script de sh (SC3039 de ShellCheck)",
            "usá $((...)) en una asignación",
            BASHISM_WHY,
            "let n=n+1 se vuelve n=$((n+1)).",
        ),
    ),
    (
        ShellRule::Sc3040,
        RuleText::new(
            "Opción de set -o fuera de POSIX en un script de sh (SC3040 de ShellCheck)",
            "sacá la opción o pasá el script a bash",
            BASHISM_WHY,
            "set -o pipefail no está en todos los sh: revisá el estado de cada comando.",
        ),
    ),
    (
        ShellRule::Sc3041,
        RuleText::new(
            "Opción corta de set fuera de POSIX en un script de sh (SC3041 de ShellCheck)",
            "sacá la opción o pasá el script a bash",
            BASHISM_WHY,
            "set -E se vuelve set -e.",
        ),
    ),
    (
        ShellRule::Sc3042,
        RuleText::new(
            "Opción larga --opción de set en un script de sh (SC3042 de ShellCheck)",
            "usá la opción corta o -o nombre",
            BASHISM_WHY,
            "set --posix se vuelve set -o posix en bash, o nada en sh.",
        ),
    ),
    (
        ShellRule::Sc3043,
        RuleText::new(
            "local en un script de sh (SC3043 de ShellCheck)",
            "usá un nombre que no choque, o corré la función en un subshell",
            BASHISM_WHY,
            "f() { local x=1; } se vuelve f() ( x=1; ).",
        ),
    ),
    (
        ShellRule::Sc3044,
        RuleText::new(
            "Comando que sólo tiene bash en un script de sh (SC3044 de ShellCheck)",
            "usá una alternativa POSIX o pasá el script a bash",
            BASHISM_WHY,
            "declare -a lista se vuelve set -- o una variable común.",
        ),
    ),
    (
        ShellRule::Sc3045,
        RuleText::new(
            "Opción de un comando fuera de POSIX en un script de sh (SC3045 de ShellCheck)",
            "usá sólo las opciones que define POSIX",
            BASHISM_WHY,
            "read -a partes se vuelve IFS=: read -r primero resto.",
        ),
    ),
    (
        ShellRule::Sc3046,
        RuleText::new(
            "source en vez de . en un script de sh (SC3046 de ShellCheck)",
            "usá .",
            BASHISM_WHY,
            "source ./env.sh se vuelve . ./env.sh.",
        ),
    ),
    (
        ShellRule::Sc3047,
        RuleText::new(
            "trap de ERR, DEBUG o RETURN en un script de sh (SC3047 de ShellCheck)",
            "atrapá EXIT o revisá el estado después de cada comando",
            BASHISM_WHY,
            "trap limpiar ERR se vuelve trap limpiar EXIT.",
        ),
    ),
    (
        ShellRule::Sc3048,
        RuleText::new(
            "Nombre de señal con el prefijo SIG en un script de sh (SC3048 de ShellCheck)",
            "sacá el prefijo SIG",
            BASHISM_WHY,
            "trap f SIGINT se vuelve trap f INT.",
        ),
    ),
    (
        ShellRule::Sc3049,
        RuleText::new(
            "Nombre de señal en minúscula o mezclado en un script de sh (SC3049 de ShellCheck)",
            "escribí el nombre de la señal en mayúsculas",
            BASHISM_WHY,
            "trap f int se vuelve trap f INT.",
        ),
    ),
    (
        ShellRule::Sc3050,
        RuleText::new(
            "printf %q en un script de sh (SC3050 de ShellCheck)",
            "poné las comillas a mano, por ejemplo con sed",
            BASHISM_WHY,
            "printf '%q' \"$x\" no tiene forma POSIX: envolvé el valor en comillas simples con sed.",
        ),
    ),
    (
        ShellRule::Sc3052,
        RuleText::new(
            "Conversión de base base#número en aritmética en un script de sh (SC3052 de ShellCheck)",
            "convertí con printf o usá el número decimal",
            BASHISM_WHY,
            "$((16#ff)) se vuelve $(printf '%d' 0xff).",
        ),
    ),
    (
        ShellRule::Sc3053,
        RuleText::new(
            "Expansión indirecta ${!var} en un script de sh (SC3053 de ShellCheck)",
            "usá eval con cuidado, o reorganizá los datos",
            BASHISM_WHY,
            "${!nombre} se vuelve eval \"valor=\\${$nombre}\".",
        ),
    ),
    (
        ShellRule::Sc3054,
        RuleText::new(
            "Referencia a un arreglo ${var[i]} en un script de sh (SC3054 de ShellCheck)",
            "usá los parámetros posicionales o variables separadas",
            BASHISM_WHY,
            "${lista[0]} se vuelve $1 después de set -- $lista.",
        ),
    ),
    (
        ShellRule::Sc3055,
        RuleText::new(
            "Expansión de claves ${!var[@]} en un script de sh (SC3055 de ShellCheck)",
            "guardá las claves en su propia lista",
            BASHISM_WHY,
            "${!lista[@]} se vuelve una variable aparte con las claves.",
        ),
    ),
    (
        ShellRule::Sc3056,
        RuleText::new(
            "Expansión de nombres por prefijo ${!prefijo*} en un script de sh (SC3056 de ShellCheck)",
            "listá las variables con set y filtralas",
            BASHISM_WHY,
            "${!APP_*} se vuelve set | grep '^APP_'.",
        ),
    ),
    (
        ShellRule::Sc3057,
        RuleText::new(
            "Recorte de string ${var:inicio:largo} en un script de sh (SC3057 de ShellCheck)",
            "usá cut, expr o un recorte con patrón",
            BASHISM_WHY,
            "${s:0:3} se vuelve $(printf '%s' \"$s\" | cut -c1-3).",
        ),
    ),
    (
        ShellRule::Sc3058,
        RuleText::new(
            "Operación de string sobre $@ o $* en un script de sh (SC3058 de ShellCheck)",
            "recorré los argumentos y aplicala a cada uno",
            BASHISM_WHY,
            "${@%.txt} se vuelve for f; do echo \"${f%.txt}\"; done.",
        ),
    ),
    (
        ShellRule::Sc3059,
        RuleText::new(
            "Cambio de mayúsculas ${var^} o ${var,,} en un script de sh (SC3059 de ShellCheck)",
            "usá tr",
            BASHISM_WHY,
            "${nombre^^} se vuelve $(printf '%s' \"$nombre\" | tr '[:lower:]' '[:upper:]').",
        ),
    ),
    (
        ShellRule::Sc3060,
        RuleText::new(
            "Reemplazo en un string ${var/a/b} en un script de sh (SC3060 de ShellCheck)",
            "usá sed",
            BASHISM_WHY,
            "${s/a/b} se vuelve $(printf '%s' \"$s\" | sed 's/a/b/').",
        ),
    ),
    (
        ShellRule::Sc3061,
        RuleText::new(
            "read sin variable en un script de sh (SC3061 de ShellCheck)",
            "nombrá la variable donde leer",
            BASHISM_WHY,
            "read -r se vuelve read -r linea.",
        ),
    ),
    (
        ShellRule::Sc3062,
        RuleText::new(
            "test -o para ver una opción del shell en un script de sh (SC3062 de ShellCheck)",
            "buscá la letra de la opción en $-",
            BASHISM_WHY,
            "[ -o errexit ] se vuelve case $- in *e*) ...;; esac.",
        ),
    ),
    (
        ShellRule::Sc3063,
        RuleText::new(
            "test -R y namerefs en un script de sh (SC3063 de ShellCheck)",
            "evitá los namerefs y pasá el valor",
            BASHISM_WHY,
            "[ -R ref ] no tiene equivalente POSIX: reorganizá el código.",
        ),
    ),
    (
        ShellRule::Sc3064,
        RuleText::new(
            "test -N en un script de sh (SC3064 de ShellCheck)",
            "compará fechas de modificación con find -newer",
            BASHISM_WHY,
            "[ -N archivo ] se vuelve un chequeo con find archivo -newer marca.",
        ),
    ),
    (
        ShellRule::Sc3065,
        RuleText::new(
            "test -k en un script de sh (SC3065 de ShellCheck)",
            "mirá el sticky bit con ls -ld o stat",
            BASHISM_WHY,
            "[ -k /tmp ] se vuelve un chequeo de la t en ls -ld /tmp.",
        ),
    ),
    (
        ShellRule::Sc3066,
        RuleText::new(
            "test -G en un script de sh (SC3066 de ShellCheck)",
            "compará el grupo del archivo con stat e id -g",
            BASHISM_WHY,
            "[ -G archivo ] se vuelve [ \"$(stat -c %g archivo)\" = \"$(id -g)\" ].",
        ),
    ),
    (
        ShellRule::Sc3067,
        RuleText::new(
            "test -O en un script de sh (SC3067 de ShellCheck)",
            "compará el dueño del archivo con stat e id -u",
            BASHISM_WHY,
            "[ -O archivo ] se vuelve [ \"$(stat -c %u archivo)\" = \"$(id -u)\" ].",
        ),
    ),
];
