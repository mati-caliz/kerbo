use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Los chequeos de Checks/Commands.hs de ShellCheck sobre builtins de la shell.
pub const SHELL_BUILTINS_TEXTS: [(ShellRule, RuleText); 18] = [
    (
        ShellRule::Sc2121,
        RuleText::new(
            "set usado para asignar una variable (SC2121 de ShellCheck)",
            "asigná con var=valor",
            "set cambia los parámetros posicionales y las opciones de la shell, no asigna variables.",
            "set nombre=valor se vuelve nombre=valor.",
        ),
    ),
    (
        ShellRule::Sc2139,
        RuleText::new(
            "alias con una expansión que se hace al definirlo (SC2139 de ShellCheck)",
            "usá comillas simples o escapá el $ para que se expanda al usarlo",
            "Entre comillas dobles la variable se lee una vez, cuando se crea el alias.",
            "alias ahora=\"date $FORMATO\" se vuelve alias ahora='date $FORMATO'.",
        ),
    ),
    (
        ShellRule::Sc2142,
        RuleText::new(
            "alias que usa parámetros posicionales (SC2142 de ShellCheck)",
            "usá una función",
            "Un alias es un reemplazo de texto: $1 es el argumento del script que lo rodea, no del alias.",
            "alias g='grep $1 archivo' se vuelve g() { grep \"$1\" archivo; }.",
        ),
    ),
    (
        ShellRule::Sc2151,
        RuleText::new(
            "return con más de un argumento (SC2151 de ShellCheck)",
            "devolvé un solo estado de 0 a 255 e imprimí lo demás",
            "return sólo acepta un código de salida; lo demás es un error.",
            "return 1 \"falló\" se vuelve echo falló >&2; return 1.",
        ),
    ),
    (
        ShellRule::Sc2152,
        RuleText::new(
            "return con algo que no es un estado de 0 a 255 (SC2152 de ShellCheck)",
            "devolvé un número de 0 a 255 e imprimí lo demás por stdout",
            "El estado de una función es un byte: los strings fallan y los números grandes dan la vuelta.",
            "return \"$nombre\" se vuelve echo \"$nombre\"; return 0.",
        ),
    ),
    (
        ShellRule::Sc2163,
        RuleText::new(
            "export de $var en vez del nombre de la variable (SC2163 de ShellCheck)",
            "sacá el $: export var",
            "export $var exporta la variable cuyo nombre es el valor de var.",
            "export $PATH se vuelve export PATH.",
        ),
    ),
    (
        ShellRule::Sc2168,
        RuleText::new(
            "local fuera de una función (SC2168 de ShellCheck)",
            "sacá el local, o pasá el código a una función",
            "local sólo funciona dentro de una función; afuera es un error.",
            "local x=1 fuera de una función se vuelve x=1.",
        ),
    ),
    (
        ShellRule::Sc2172,
        RuleText::new(
            "trap de una señal por número (SC2172 de ShellCheck)",
            "usá el nombre de la señal",
            "Los números de señal que no son los clásicos cambian entre sistemas.",
            "trap limpiar 10 se vuelve trap limpiar USR1.",
        ),
    ),
    (
        ShellRule::Sc2173,
        RuleText::new(
            "trap de SIGKILL o SIGSTOP, que no se pueden atrapar (SC2173 de ShellCheck)",
            "sacá esa señal del trap",
            "El kernel nunca le entrega KILL ni STOP al proceso, así que el manejador no corre.",
            "trap limpiar INT KILL se vuelve trap limpiar INT.",
        ),
    ),
    (
        ShellRule::Sc2184,
        RuleText::new(
            "Argumento de unset sin comillas que la shell puede expandir (SC2184 de ShellCheck)",
            "poné el argumento entre comillas",
            "unset arr[0] puede coincidir con un archivo arr0 y borrar esa variable en su lugar.",
            "unset arr[0] se vuelve unset 'arr[0]'.",
        ),
    ),
    (
        ShellRule::Sc2219,
        RuleText::new(
            "let en vez de (( )) (SC2219 de ShellCheck)",
            "usá (( expresión ))",
            "(( )) hace la misma aritmética sin los problemas de comillas de los argumentos de let.",
            "let 'n = n + 1' se vuelve (( n = n + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2229,
        RuleText::new(
            "read en $var en vez del nombre de la variable (SC2229 de ShellCheck)",
            "sacá el $: read var",
            "read $var lee en la variable cuyo nombre es el valor de var.",
            "read $linea se vuelve read -r linea.",
        ),
    ),
    (
        ShellRule::Sc2240,
        RuleText::new(
            ". con argumentos, que sh y dash no pasan (SC2240 de ShellCheck)",
            "dejá los valores en variables antes de cargar el archivo",
            "En sh y dash el comando punto ignora los argumentos de más: el archivo no los ve.",
            ". ./lib.sh debug se vuelve MODO=debug . ./lib.sh.",
        ),
    ),
    (
        ShellRule::Sc2241,
        RuleText::new(
            "exit con más de un argumento (SC2241 de ShellCheck)",
            "salí con un solo estado de 0 a 255 e imprimí lo demás",
            "exit sólo acepta un código de salida; lo demás es un error.",
            "exit 1 \"falló\" se vuelve echo falló >&2; exit 1.",
        ),
    ),
    (
        ShellRule::Sc2242,
        RuleText::new(
            "exit con algo que no es un estado de 0 a 255 (SC2242 de ShellCheck)",
            "salí con un número de 0 a 255 e imprimí lo demás por stderr",
            "El estado de salida es un byte: los strings fallan y los números grandes dan la vuelta.",
            "exit -1 se vuelve exit 1.",
        ),
    ),
    (
        ShellRule::Sc2293,
        RuleText::new(
            "eval de un arreglo con @Q y el índice @ (SC2293 de ShellCheck)",
            "usá * como índice",
            "Con @ cada elemento es una palabra aparte para eval; * junta las palabras con comillas en un string.",
            "eval \"${arr[@]@Q}\" se vuelve eval \"${arr[*]@Q}\".",
        ),
    ),
    (
        ShellRule::Sc2294,
        RuleText::new(
            "eval de un arreglo, que pierde sus comillas (SC2294 de ShellCheck)",
            "corré el arreglo directamente, o hacé eval de un solo string",
            "eval junta los elementos y los vuelve a parsear, así que los espacios y símbolos se rompen.",
            "eval \"${cmd[@]}\" se vuelve \"${cmd[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2313,
        RuleText::new(
            "Índice de arreglo sin comillas en read, que la shell puede expandir (SC2313 de ShellCheck)",
            "poné el índice entre comillas",
            "read arr[0] puede coincidir con un archivo arr0 y leer en ese nombre.",
            "read arr[0] se vuelve read 'arr[0]'.",
        ),
    ),
];
