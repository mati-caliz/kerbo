use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de Analytics.hs de ShellCheck sobre los operandos de [ ] y [[ ]].
pub const TEST_OPERANDS_TEXTS: [(ShellRule, RuleText); 23] = [
    (
        ShellRule::Sc2050,
        RuleText::new(
            "Comparación entre dos constantes (SC2050 de ShellCheck)",
            "agregá el $ que le falta a la variable",
            "El resultado es siempre el mismo, así que la condición no decide nada.",
            "[ n -le 4 ] se vuelve [ \"$n\" -le 4 ].",
        ),
    ),
    (
        ShellRule::Sc2076,
        RuleText::new(
            "Lado derecho de =~ entre comillas (SC2076 de ShellCheck)",
            "sacá las comillas para que se lea como expresión regular",
            "Con comillas el texto se compara tal cual, no como expresión.",
            "[[ $a =~ \"x.*\" ]] se vuelve [[ $a =~ x.* ]].",
        ),
    ),
    (
        ShellRule::Sc2077,
        RuleText::new(
            "Comparación sin espacios alrededor del operador (SC2077 de ShellCheck)",
            "poné espacios alrededor del operador",
            "Sin espacios es una sola palabra, y una palabra no vacía siempre da verdadero.",
            "[ $a=3 ] se vuelve [ \"$a\" = 3 ].",
        ),
    ),
    (
        ShellRule::Sc2078,
        RuleText::new(
            "Condición con una palabra constante (SC2078 de ShellCheck)",
            "agregá el $ que falta, o usá el comando sin corchetes",
            "Una palabra no vacía siempre da verdadero.",
            "[ cmd ] se vuelve cmd.",
        ),
    ),
    (
        ShellRule::Sc2143,
        RuleText::new(
            "Se compara la salida de grep en vez de usar -q (SC2143 de ShellCheck)",
            "usá grep -q como condición",
            "grep -q corta en la primera coincidencia y no arma un texto.",
            "[ -n \"$(grep x f)\" ] se vuelve grep -q x f.",
        ),
    ),
    (
        ShellRule::Sc2144,
        RuleText::new(
            "Prueba de archivo sobre un glob (SC2144 de ShellCheck)",
            "usá un for",
            "Con varias coincidencias la prueba recibe argumentos de más.",
            "[ -e *.txt ] se vuelve for f in *.txt; do [ -e \"$f\" ]; done.",
        ),
    ),
    (
        ShellRule::Sc2157,
        RuleText::new(
            "-n o -z sobre una palabra que siempre tiene texto (SC2157 de ShellCheck)",
            "sacá el texto fijo del operando, o la prueba",
            "Con texto fijo en la palabra, el resultado es siempre el mismo.",
            "[ -n \"$a \" ] se vuelve [ -n \"$a\" ].",
        ),
    ),
    (
        ShellRule::Sc2158,
        RuleText::new(
            "[ false ] da verdadero (SC2158 de ShellCheck)",
            "sacá los corchetes",
            "Cualquier palabra no vacía adentro de [ ] da verdadero, también la palabra false.",
            "[ false ] se vuelve false.",
        ),
    ),
    (
        ShellRule::Sc2159,
        RuleText::new(
            "[ 0 ] da verdadero (SC2159 de ShellCheck)",
            "usá false",
            "Cualquier palabra no vacía adentro de [ ] da verdadero, también un cero.",
            "[ 0 ] se vuelve false.",
        ),
    ),
    (
        ShellRule::Sc2160,
        RuleText::new(
            "[ true ] en vez de true (SC2160 de ShellCheck)",
            "usá true sin corchetes",
            "Los corchetes no le suman nada al comando.",
            "[ true ] se vuelve true.",
        ),
    ),
    (
        ShellRule::Sc2161,
        RuleText::new(
            "[ 1 ] en vez de true (SC2161 de ShellCheck)",
            "usá true",
            "Parece un número, pero es sólo una palabra no vacía.",
            "[ 1 ] se vuelve true.",
        ),
    ),
    (
        ShellRule::Sc2193,
        RuleText::new(
            "Comparación que nunca puede dar igual (SC2193 de ShellCheck)",
            "revisá el texto de los dos lados",
            "Las partes fijas de los dos lados no coinciden con ningún valor.",
            "[[ x$a == y* ]] nunca coincide.",
        ),
    ),
    (
        ShellRule::Sc2198,
        RuleText::new(
            "Arreglo como operando de [ ] (SC2198 de ShellCheck)",
            "usá un loop, o concatená con * en vez de @",
            "Cada elemento se vuelve un argumento aparte de la prueba.",
            "[ \"$@\" = x ] falla con varios argumentos.",
        ),
    ),
    (
        ShellRule::Sc2199,
        RuleText::new(
            "Arreglo en [[ ]], que lo concatena (SC2199 de ShellCheck)",
            "usá un loop, o un * explícito en vez de @",
            "Los elementos se juntan en una palabra y la comparación los mira a todos juntos.",
            "[[ $@ = x ]] compara la lista entera.",
        ),
    ),
    (
        ShellRule::Sc2200,
        RuleText::new(
            "Expansión de llaves como operando de [ ] (SC2200 de ShellCheck)",
            "usá un loop",
            "Cada expansión se vuelve un argumento aparte de la prueba.",
            "[ $a = {x,y} ] recibe argumentos de más.",
        ),
    ),
    (
        ShellRule::Sc2201,
        RuleText::new(
            "Expansión de llaves en [[ ]], donde no ocurre (SC2201 de ShellCheck)",
            "usá un loop",
            "El texto se compara con las llaves incluidas.",
            "[[ $a == {x,y} ]] nunca se expande.",
        ),
    ),
    (
        ShellRule::Sc2202,
        RuleText::new(
            "Glob como operando de [ ] (SC2202 de ShellCheck)",
            "usá un loop",
            "Cada coincidencia se vuelve un argumento aparte de la prueba.",
            "[ *.txt ] recibe argumentos de más con varios archivos.",
        ),
    ),
    (
        ShellRule::Sc2203,
        RuleText::new(
            "Glob en [[ ]] donde no se expande (SC2203 de ShellCheck)",
            "usá un loop",
            "Los globs sólo valen a la derecha de = y != en [[ ]].",
            "[[ *.txt == a ]] compara el texto *.txt.",
        ),
    ),
    (
        ShellRule::Sc2208,
        RuleText::new(
            "-v con un glob en [ ] (SC2208 de ShellCheck)",
            "usá [[ ]] o poné el argumento entre comillas",
            "El nombre se puede expandir a archivos del directorio actual.",
            "[ -v a[0] ] se vuelve [[ -v a[0] ]].",
        ),
    ),
    (
        ShellRule::Sc2245,
        RuleText::new(
            "Prueba de archivo sobre un glob en ksh, que sólo mira la primera coincidencia (SC2245 de ShellCheck)",
            "usá un loop para mirar alguna o todas",
            "La prueba se aplica sólo al primer archivo de la expansión.",
            "[ -d foo* ] mira un solo directorio.",
        ),
    ),
    (
        ShellRule::Sc2255,
        RuleText::new(
            "Comparación numérica con un glob en [ ] (SC2255 de ShellCheck)",
            "usá $((..)) para números, o una comparación de texto",
            "[ ] no evalúa aritmética, así que el * es un glob.",
            "[ 6 -eq 2*3 ] se vuelve [ 6 -eq $((2*3)) ].",
        ),
    ),
    (
        ShellRule::Sc2268,
        RuleText::new(
            "Prefijo x en una comparación (SC2268 de ShellCheck)",
            "compará los valores directamente",
            "Los shells que lo necesitaban ya no existen hace décadas.",
            "[ x$a = xb ] se vuelve [ \"$a\" = b ].",
        ),
    ),
    (
        ShellRule::Sc2075,
        RuleText::new(
            "\\< o \\> escapado en [[ ]] (SC2075 de ShellCheck)",
            "sacá la barra",
            "[[ ]] compara con < y > tal cual; la forma escapada sólo va en [ ].",
            "[[ a \\< b ]] se vuelve [[ a < b ]].",
        ),
    ),
];
