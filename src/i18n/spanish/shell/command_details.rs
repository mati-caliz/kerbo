use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de Checks/Commands.hs y Checks/ShellSupport.hs de ShellCheck sobre el detalle de un comando: el formato de printf, el case de getopts, las declaraciones y lo que soporta cada shell.
pub const COMMAND_DETAILS_TEXTS: [(ShellRule, RuleText); 19] = [
    (
        ShellRule::Sc2025,
        RuleText::new(
            "Secuencia de escape en PS1 afuera de \\[..\\] (SC2025 de ShellCheck)",
            "encerrá las secuencias en \\[..\\]",
            "Si no, el largo del prompt se cuenta mal y las líneas largas se cortan mal.",
            "PS1='\\e[0m$ ' se vuelve PS1='\\[\\e[0m\\]$ '.",
        ),
    ),
    (
        ShellRule::Sc2051,
        RuleText::new(
            "Variable en un rango de llaves (SC2051 de ShellCheck)",
            "usá seq o un for ((..))",
            "Bash expande las llaves antes que las variables, y el rango queda como texto.",
            "{1..$n} se vuelve $(seq 1 \"$n\").",
        ),
    ),
    (
        ShellRule::Sc2175,
        RuleText::new(
            "Rango de llaves con una variable pasado a eval (SC2175 de ShellCheck)",
            "ponelo entre comillas, porque tiene que llegar literal a eval",
            "Sin comillas se lee dos veces.",
            "eval echo {1..$n} se vuelve eval echo \"{1..$n}\".",
        ),
    ),
    (
        ShellRule::Sc2176,
        RuleText::new(
            "time con un pipeline en sh (SC2176 de ShellCheck)",
            "medí una sola etapa, o usá bash -c",
            "Ahí time no está definido para pipelines.",
            "time a | b se vuelve bash -c 'time a | b'.",
        ),
    ),
    (
        ShellRule::Sc2177,
        RuleText::new(
            "time con un comando compuesto en sh (SC2177 de ShellCheck)",
            "usá time sh -c",
            "Ahí time no está definido para comandos compuestos.",
            "time { a; } se vuelve time sh -c 'a'.",
        ),
    ),
    (
        ShellRule::Sc2180,
        RuleText::new(
            "Arreglo de varias dimensiones en bash (SC2180 de ShellCheck)",
            "usá un arreglo simple o uno asociativo",
            "Bash no los tiene.",
            "a=([1][2]=3) se vuelve a=([1,2]=3).",
        ),
    ),
    (
        ShellRule::Sc2182,
        RuleText::new(
            "Formato de printf sin variables pero con argumentos (SC2182 de ShellCheck)",
            "agregá el formato, o sacá los argumentos",
            "Los argumentos se ignoran y el texto se repite.",
            "printf foo bar se vuelve printf '%s %s' foo bar.",
        ),
    ),
    (
        ShellRule::Sc2183,
        RuleText::new(
            "Formato de printf con otra cantidad de variables que de argumentos (SC2183 de ShellCheck)",
            "hacé coincidir el formato y los argumentos",
            "Los valores que faltan quedan vacíos, y los de más repiten el formato.",
            "printf '%s %s' a se vuelve printf '%s %s' a b.",
        ),
    ),
    (
        ShellRule::Sc2213,
        RuleText::new(
            "Opción de getopts que el case no maneja (SC2213 de ShellCheck)",
            "agregá la opción al case, o sacala de getopts",
            "La opción se acepta y no pasa nada.",
            "getopts 'ab' con sólo a) necesita b).",
        ),
    ),
    (
        ShellRule::Sc2214,
        RuleText::new(
            "Rama de case para una opción que getopts no declara (SC2214 de ShellCheck)",
            "agregá la opción a getopts, o sacá la rama",
            "getopts nunca devuelve esa opción.",
            "getopts 'a' con b) necesita getopts 'ab'.",
        ),
    ),
    (
        ShellRule::Sc2220,
        RuleText::new(
            "case de getopts sin rama para opciones inválidas (SC2220 de ShellCheck)",
            "agregá una rama *)",
            "Una opción inválida pasa sin aviso.",
            "case $o in a) x;; esac necesita *) usage;;.",
        ),
    ),
    (
        ShellRule::Sc2290,
        RuleText::new(
            "Espacios alrededor del = en una declaración (SC2290 de ShellCheck)",
            "sacá los espacios",
            "El valor es otro argumento: la variable no lo recibe.",
            "export a =b se vuelve export a=b.",
        ),
    ),
    (
        ShellRule::Sc2291,
        RuleText::new(
            "Espacios repetidos entre argumentos de echo (SC2291 de ShellCheck)",
            "ponelos entre comillas",
            "Sin comillas se juntan en uno.",
            "echo a    b se vuelve echo \"a    b\".",
        ),
    ),
    (
        ShellRule::Sc2302,
        RuleText::new(
            "for sobre valores cuya variable se usa como índice (SC2302 de ShellCheck)",
            "para recorrer las claves, usá \"${!array[@]}\"",
            "La variable tiene los valores, no las posiciones.",
            "for i in \"${a[@]}\" se vuelve for i in \"${!a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2303,
        RuleText::new(
            "Valor de un arreglo usado como clave (SC2303 de ShellCheck)",
            "usá el valor directo, o recorré las claves",
            "La variable tiene los valores, no las posiciones.",
            "${a[$i]} se vuelve $i.",
        ),
    ),
    (
        ShellRule::Sc2318,
        RuleText::new(
            "Asignación usada de nuevo en la misma declaración (SC2318 de ShellCheck)",
            "usá dos declaraciones",
            "El valor todavía no tomó efecto.",
            "declare a=1 b=$a se vuelve declare a=1; declare b=$a.",
        ),
    ),
    (
        ShellRule::Sc2325,
        RuleText::new(
            "Varios ! delante de un pipeline en sh (SC2325 de ShellCheck)",
            "usá uno como máximo",
            "Es una extensión de bash y ksh.",
            "! ! a se vuelve a.",
        ),
    ),
    (
        ShellRule::Sc2326,
        RuleText::new(
            "! en el medio de un pipeline (SC2326 de ShellCheck)",
            "usá un grupo: cmd | { ! cmd; }",
            "Ahí no se permite.",
            "a | ! b se vuelve a | { ! b; }.",
        ),
    ),
    (
        ShellRule::Sc2332,
        RuleText::new(
            "[ ! -a x ] o [ ! -o x ] en bash (SC2332 de ShellCheck)",
            "usá -e, o ! [ -o opt ]",
            "Con !, -a y -o se leen como y y o lógicos: el resultado siempre es verdadero.",
            "[ ! -a f ] se vuelve [ ! -e f ].",
        ),
    ),
];
