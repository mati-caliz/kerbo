use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

const SPLITTING_WHY: &str = "Sin comillas, el shell parte el valor en cada espacio y expande los comodines: un nombre de archivo con un espacio o un asterisco se vuelve varios argumentos.";
const USELESS_ECHO_WHY: &str =
    "El echo sólo repite lo que ya tenés: suma un proceso y, sin comillas, parte y expande el texto.";

const UNKNOWN: RuleText = RuleText::new(
    "Regla de ShellCheck",
    "seguí la indicación de la regla",
    "ShellCheck la marca por defecto.",
    "Ver la wiki de ShellCheck.",
);
const TEXTS: [RuleText; ShellRule::ALL.len()] = [
    RuleText::new(
        "$ o ${} de más en una variable dentro de una expresión aritmética",
        "sacá el $: dentro de $(( )) las variables se leen por nombre",
        "En una expresión aritmética el nombre ya es la variable; con $ el valor se pega como texto antes de evaluar y un valor vacío rompe la expresión.",
        "$(( $total + 1 )) se vuelve $(( total + 1 )).",
    ),
    RuleText::new(
        "echo $(comando): el echo sobra",
        "corré el comando directamente",
        USELESS_ECHO_WHY,
        "echo \"$(date)\" se vuelve date.",
    ),
    RuleText::new(
        "Sustitución de comandos con comillas invertidas",
        "usá $(...) en vez de `...`",
        "Las comillas invertidas no se anidan sin escapes y tratan las barras de forma distinta; $(...) se lee y se anida sin sorpresas.",
        "fecha=`date` se vuelve fecha=$(date).",
    ),
    RuleText::new(
        "A && B || C no es un if: C también corre si B falla",
        "usá if A; then B; else C; fi",
        "Si A sale bien pero B falla, C corre igual: el atajo esconde un camino que nadie quiso escribir.",
        "[ -f x ] && cp x y || echo falta se vuelve if [ -f x ]; then cp x y; else echo falta; fi.",
    ),
    RuleText::new(
        "Una expansión entre comillas simples no se expande",
        "usá comillas dobles si querés el valor, o escapá el $ si querés el texto",
        "Entre comillas simples $variable y $(comando) quedan literales: lo más probable es que se esperara el valor.",
        "echo 'Hola $USER' se vuelve echo \"Hola $USER\".",
    ),
    RuleText::new(
        "Una variable que se asigna y nadie lee",
        "borrala, usala o exportala si la lee otro programa",
        "Una asignación que nadie usa suele ser un nombre mal escrito o código que quedó de otra versión.",
        "Si sólo la lee un script que te incluye, export LOG_LEVEL=debug lo deja explícito.",
    ),
    RuleText::new(
        "Una sustitución de comandos sin comillas",
        "poné comillas alrededor del $(...)",
        SPLITTING_WHY,
        "rm $(ls *.tmp) se vuelve rm \"$(ls *.tmp)\", o mejor un for sobre el glob.",
    ),
    RuleText::new(
        "Una variable como formato de printf",
        "usá un formato fijo y pasá la variable como argumento",
        "Un % o una barra en el valor se interpretan como formato: la salida cambia según los datos.",
        "printf \"$mensaje\" se vuelve printf '%s\\n' \"$mensaje\".",
    ),
    RuleText::new(
        "trap con comillas dobles: se expande al declararlo",
        "usá comillas simples para que se expanda cuando llega la señal",
        "Con comillas dobles el comando del trap guarda los valores de hoy, no los que haya cuando corra.",
        "trap \"rm -f $tmp\" EXIT se vuelve trap 'rm -f \"$tmp\"' EXIT.",
    ),
    RuleText::new(
        "Una expansión de arreglo sin comillas",
        "poné comillas: \"$@\" o \"${arreglo[@]}\"",
        "Sin comillas cada elemento se vuelve a partir en espacios y comodines: un argumento con espacios llega como varios.",
        "comando $@ se vuelve comando \"$@\".",
    ),
    RuleText::new(
        "2>&1 antes de la redirección a archivo",
        "poné el 2>&1 al final: > archivo 2>&1",
        "Las redirecciones se aplican en orden: 2>&1 primero apunta stderr a la terminal y recién después stdout va al archivo.",
        "comando 2>&1 > log se vuelve comando > log 2>&1.",
    ),
    RuleText::new(
        "Una variable sin comillas: se parte en palabras y se expanden comodines",
        "poné comillas dobles alrededor de la variable",
        SPLITTING_WHY,
        "cp $origen $destino se vuelve cp \"$origen\" \"$destino\".",
    ),
    RuleText::new(
        "$(echo ...): el echo sobra",
        "usá el valor directamente",
        USELESS_ECHO_WHY,
        "nombre=$(echo $base) se vuelve nombre=$base.",
    ),
    RuleText::new(
        "El script no dice qué shell lo corre",
        "agregá un shebang en la primera línea, o una directiva shell=",
        "Sin shebang, el script corre con el shell de quien lo llama y las reglas no saben si es sh o bash.",
        "Primera línea: #!/usr/bin/env bash",
    ),
    RuleText::new(
        "Declarar y asignar juntos esconde el código de salida",
        "declarala en una línea y asignala en la siguiente",
        "local, export o declare devuelven su propio éxito: si el comando de la asignación falla, nadie se entera, ni con set -e.",
        "local version=$(git describe) se vuelve local version y después version=$(git describe).",
    ),
    RuleText::new(
        "read sin -r rompe las barras invertidas",
        "agregá -r al read",
        "Sin -r, read se come las barras: una ruta de Windows o una línea con \\n llega cambiada.",
        "read linea se vuelve read -r linea.",
    ),
    RuleText::new(
        "cd sin manejar el error",
        "agregá || exit (o || return dentro de una función)",
        "Si el cd falla, el script sigue en la carpeta anterior y los comandos que siguen corren donde no deben: un rm puede borrar otra cosa.",
        "cd \"$destino\" se vuelve cd \"$destino\" || exit.",
    ),
    RuleText::new(
        "-a o -o dentro de [ ]",
        "partí la condición: [ p ] && [ q ] o [ p ] || [ q ]",
        "POSIX marca -a y -o como obsoletos y su precedencia es ambigua: según los valores, test lee otra cosa.",
        "[ -n \"$a\" -a -n \"$b\" ] se vuelve [ -n \"$a\" ] && [ -n \"$b\" ].",
    ),
    RuleText::new(
        "Revisar $? en vez del comando",
        "poné el comando directamente en el if",
        "Entre el comando y el chequeo de $? se puede colar otro comando, y el código queda más largo y frágil.",
        "comando; if [ $? -ne 0 ]; then ... se vuelve if ! comando; then ...",
    ),
    RuleText::new(
        "Un arreglo armado con una variable sin comillas",
        "poné comillas, o partí con read -a o mapfile",
        SPLITTING_WHY,
        "partes=( $linea ) se vuelve read -ra partes <<< \"$linea\".",
    ),
    RuleText::new(
        "Un arreglo armado con la salida de un comando sin comillas",
        "usá mapfile o read -a para partir la salida",
        SPLITTING_WHY,
        "archivos=( $(ls) ) se vuelve mapfile -t archivos < <(ls).",
    ),
    RuleText::new(
        "Una asignación por defecto sin comillas en un argumento de :",
        "poné comillas: : \"${variable:=valor}\"",
        "Sin comillas el valor se parte y se expanden comodines: un * en el default recorre el disco entero.",
        ": ${DIR:=/tmp} se vuelve : \"${DIR:=/tmp}\".",
    ),
];

pub fn shell(rule: ShellRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
